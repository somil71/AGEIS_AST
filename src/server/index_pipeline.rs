//! Reusable indexing pipeline — called by the background indexer for cloud repos.

use crate::{
    chunking::detect_chunker,
    config::Config,
    embedding::EmbeddingModel,
    graph,
    indexing::Index,
    schema::{IndexMetadata, Language},
    storage::Storage,
};
use chrono::Utc;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Index `source_dir` and store the result at `index_dir`.
/// Silent (no progress bars) — designed to run in a background task.
pub fn run(source_dir: &Path, index_dir: &Path) -> crate::Result<IndexStats> {
    let config = Config {
        watched_dirs: vec![source_dir.to_string_lossy().to_string()],
        ..Config::default()
    };

    let storage = Storage::new(index_dir.to_path_buf())?;

    // Collect files
    let all_files = collect_files(source_dir, &config);
    if all_files.is_empty() {
        return Err(crate::Error::InvalidPath("No supported files found".to_string()));
    }

    let embed_model = EmbeddingModel::new(config.embedding_dim)?;

    let mut index = Index::with_params(
        config.embedding_dim,
        config.hnsw_m as usize,
        config.hnsw_ef_construction as usize,
        config.bm25_k1,
        config.bm25_b,
    );

    let mut filemap: HashMap<String, Vec<u64>> = HashMap::new();
    let mut file_hashes: HashMap<String, u64> = HashMap::new();

    // Load existing index if it exists for incremental update
    if Storage::index_exists() {
        if let Ok(bm25) = storage.load_bm25() { index.inverted = bm25; }
        if let Ok(hnsw) = storage.load_hnsw() { index.hnsw = hnsw; }
        if let Ok(chunks) = storage.load_chunks() {
            let max_id = chunks.keys().copied().max().unwrap_or(0);
            index.chunk_store = chunks;
            index.set_next_id(max_id + 1);
        }
        if let Ok(fm) = storage.load_filemap() { filemap = fm; }
        if let Ok(fh) = storage.load_file_hashes() { file_hashes = fh; }
    }

    // Identify changed files and deleted files
    let mut current_paths = std::collections::HashSet::new();
    let mut changed_files = Vec::new();
    
    for (path, lang) in all_files {
        let path_str = path.to_string_lossy().to_string();
        current_paths.insert(path_str.clone());
        
        let content = if lang == Language::Pdf {
            pdf_extract::extract_text(&path).unwrap_or_default()
        } else {
            std::fs::read_to_string(&path).unwrap_or_default()
        };
        
        if content.is_empty() { continue; }
        
        let hash = xxhash_rust::xxh3::xxh3_64(content.as_bytes());
        
        if file_hashes.get(&path_str) == Some(&hash) {
            continue; // Unchanged!
        }
        
        // Changed or new
        changed_files.push((path, lang, content, hash, path_str));
    }
    
    // Remove deleted or changed files from index
    let paths_to_remove: Vec<String> = filemap.keys()
        .filter(|p| !current_paths.contains(*p) || changed_files.iter().any(|(_, _, _, _, ps)| ps == *p))
        .cloned()
        .collect();
        
    for p in paths_to_remove {
        if let Some(chunk_ids) = filemap.remove(&p) {
            for cid in chunk_ids {
                let _ = index.delete_chunk(cid);
            }
        }
        file_hashes.remove(&p);
    }

    // Process changed files (Parallel Chunking)
    let raw_chunks: Vec<(crate::schema::Chunk, String, u64)> = changed_files
        .par_iter()
        .flat_map_iter(|(path, lang, content, hash, path_str)| {
            let chunker = detect_chunker(*lang);
            let chunks = chunker.chunk(content, path, *lang).unwrap_or_default();
            chunks.into_iter().map(move |c| (c, path_str.clone(), *hash))
        })
        .collect();

    let enrich_acronyms = std::env::var("NEEDLE_ENRICH_ACRONYMS").is_ok();

    for (mut chunk, path_str, hash) in raw_chunks {
        if enrich_acronyms && chunk.chunk_type == crate::schema::ChunkType::Function {
            let expanded = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    crate::llm::expand_legacy_vocabulary(&chunk.content).await
                })
            });
            if let Ok(terms) = expanded {
                if !terms.is_empty() {
                    chunk.content.push_str("
// Semantic Expansions: ");
                    chunk.content.push_str(&terms);
                }
            }
        }

        let chunk_id = index.next_id();
        chunk.id = chunk_id;
        chunk.embedding_id = chunk_id;
        let embedding = embed_model.embed(&chunk.content);
        
        filemap.entry(path_str.clone()).or_default().push(chunk_id);
        file_hashes.insert(path_str, hash);
        index.add_chunk(chunk, embedding)?;
    }

    // Save to disk
    storage.save_bm25(&index.inverted)?;
    storage.save_hnsw(&index.hnsw)?;
    storage.save_chunks(&index.chunk_store)?;
    storage.save_filemap(&filemap)?;
    storage.save_file_hashes(&file_hashes)?;

    // Regenerate graph
    // (In a real incremental setup, graph generation would also be incremental, but for now we regenerate from chunk_store)
    let mut all_file_entries = Vec::new();
    for (path_str, chunk_ids) in &filemap {
        let mut content = String::new();
        let lang = Language::PlainText; // We just need content for graph extraction
        for cid in chunk_ids {
            if let Some(c) = index.chunk_store.get(cid) {
                content.push_str(&c.content);
                content.push_str("
");
            }
        }
        all_file_entries.push((PathBuf::from(path_str), lang, content));
    }
    
    let code_graph = graph::extract(&all_file_entries);
    storage.save_graph(&code_graph)?;

    let total_chunks = index.total_chunks();
    let total_files = filemap.len();

    let meta = IndexMetadata {
        total_chunks: total_chunks as u64,
        total_files: total_files as u64,
        last_update_ts: Utc::now().timestamp() as u64,
        embedding_model: embed_model.model_string(),
        embedding_dim: config.embedding_dim as u32,
        hnsw_m: config.hnsw_m,
        hnsw_ef_construction: config.hnsw_ef_construction,
        bm25_k1: config.bm25_k1,
        bm25_b: config.bm25_b,
        watched_dirs: config.watched_dirs.clone(),
        avg_chunk_length: index.inverted.avg_doc_length(),
        ..IndexMetadata::default()
    };
    storage.save_metadata(&meta)?;
    
    // Auto-sign ledger block for Sovereign Audit Ledger Automation (Phase 3)
    if let Ok(kp) = crate::ledger::LedgerKeypair::load_from_file(&crate::ledger::default_key_priv_path()) {
        let _ = crate::ledger::append_to_ledger(
            &crate::ledger::default_ledger_path(),
            &kp,
            std::str::FromStr::from_str("compliance_audit").unwrap(),
            serde_json::json!({
                "event": "incremental_index_completed",
                "stats": {
                    "total_chunks": total_chunks,
                    "total_files": total_files,
                    "changed_files_processed": changed_files.len()
                },
                "timestamp": Utc::now().timestamp()
            })
        );
    }

    Ok(IndexStats { total_chunks, total_files })
}

pub struct IndexStats {
    pub total_chunks: usize,
    pub total_files:  usize,
}

fn collect_files(dir: &Path, config: &Config) -> Vec<(PathBuf, Language)> {
    WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.')
                && !config.should_ignore(&name)
                && !config.should_ignore(&e.path().to_string_lossy())
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let path = clean_path(e.path().to_path_buf());
            let ext = path.extension()?.to_str()?;
            let lang = Language::from_extension(ext)?;
            let meta = std::fs::metadata(&path).ok()?;
            let size_limit = if lang == Language::Pdf { 20_000_000 } else { 1_000_000 };
            if meta.len() > size_limit { return None; }
            Some((path, lang))
        })
        .collect()
}

/// Strip Windows extended-length path prefix `\\?\` so paths are consistent.
fn clean_path(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}
