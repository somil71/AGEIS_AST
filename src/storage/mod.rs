//! Storage layer: persistence for all index components.
//!
//! Layout under `~/.needle/`:
//!   config.toml          — user config
//!   index/
//!     meta.json          — IndexMetadata (snapshot sequence, stats)
//!     chunks.json        — HashMap<u64, Chunk>
//!     bm25.bin           — BM25Index (bincode)
//!     hnsw.bin           — HnswIndex (bincode, includes flat embeddings)
//!     filemap.json       — HashMap<path, Vec<chunk_id>>
//!     wal/               — write-ahead log (future)

use crate::config::Config;
use crate::graph::CodeGraph;
use crate::indexing::{bm25::BM25Index, hnsw::HnswIndex};
use crate::schema::{Chunk, IndexMetadata};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Storage {
    pub index_dir: PathBuf,
}

impl Storage {
    pub fn new(index_dir: PathBuf) -> crate::Result<Self> {
        std::fs::create_dir_all(&index_dir)?;
        std::fs::create_dir_all(index_dir.join("wal"))?;
        
        // Ensure .needle is gitignored to prevent ledger key and index leakage
        if let Some(parent) = index_dir.parent() {
            let gitignore = parent.join(".gitignore");
            if !gitignore.exists() {
                let _ = std::fs::write(gitignore, "*\n");
            }
        }
        
        Ok(Self { index_dir })
    }

    /// Returns `~/.needle/`
    pub fn needle_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".needle")
    }

    /// Walks up from the current directory looking for a `.git`, so the
    /// index resolves to the repo root rather than whatever subdirectory you
    /// happen to be in. Returns `None` if there's no git repo above cwd —
    /// callers fall back to the current directory itself in that case.
    fn find_git_root() -> Option<PathBuf> {
        let mut dir = std::env::current_dir().ok()?;
        loop {
            if dir.join(".git").exists() {
                return Some(dir);
            }
            if !dir.pop() {
                return None;
            }
        }
    }

    /// Returns the index directory for the current project.
    ///
    /// Always scoped to the working directory: `<project_root>/.needle/index`,
    /// where `project_root` is the nearest git root above cwd, or cwd itself
    /// if there's no `.git` anywhere above it (e.g. a project that hasn't
    /// been git-initialized yet). This is deliberately *not* a single global
    /// path — every distinct project directory gets its own index, so
    /// indexing one codebase never silently overwrites another's.
    pub fn default_index_dir() -> PathBuf {
        let root = Self::find_git_root()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        root.join(".needle").join("index")
    }

    /// Returns `~/.needle/config.toml`
    pub fn config_path() -> PathBuf {
        Self::needle_dir().join("config.toml")
    }

    pub fn index_exists() -> bool {
        let meta = Self::default_index_dir().join("meta.json");
        meta.exists()
    }

    // -----------------------------------------------------------------------
    // Config
    // -----------------------------------------------------------------------

    pub fn save_config(config: &Config) -> crate::Result<()> {
        let needle_dir = Self::needle_dir();
        std::fs::create_dir_all(&needle_dir)?;
        let path = Self::config_path();
        let content = toml::to_string_pretty(config)
            .map_err(|e| crate::error::Error::ConfigError(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn load_config() -> crate::Result<Config> {
        let path = Self::config_path();
        if !path.exists() {
            return Err(crate::error::Error::ConfigError(
                "Config not found. Run: needle init <dirs...>".to_string(),
            ));
        }
        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    // -----------------------------------------------------------------------
    // Index metadata
    // -----------------------------------------------------------------------

    pub fn save_metadata(&self, meta: &IndexMetadata) -> crate::Result<()> {
        let path = self.index_dir.join("meta.json");
        let json = serde_json::to_string_pretty(meta)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load_metadata(&self) -> crate::Result<IndexMetadata> {
        let path = self.index_dir.join("meta.json");
        let content = std::fs::read_to_string(&path)?;
        let meta: IndexMetadata = serde_json::from_str(&content)?;
        Ok(meta)
    }

    // -----------------------------------------------------------------------
    // Chunks store
    // -----------------------------------------------------------------------

    pub fn save_chunks(&self, chunks: &HashMap<u64, Chunk>) -> crate::Result<()> {
        let path = self.index_dir.join("chunks.json");
        let json = serde_json::to_string(chunks)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load_chunks(&self) -> crate::Result<HashMap<u64, Chunk>> {
        let path = self.index_dir.join("chunks.json");
        let content = std::fs::read_to_string(&path)?;
        let chunks: HashMap<u64, Chunk> = serde_json::from_str(&content)?;
        Ok(chunks)
    }

    // -----------------------------------------------------------------------
    // BM25 index (bincode for efficiency)
    // -----------------------------------------------------------------------

    pub fn save_bm25(&self, index: &BM25Index) -> crate::Result<()> {
        let path = self.index_dir.join("bm25.bin");
        let bytes = bincode::serialize(index)
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        std::fs::write(&path, bytes)?;
        Ok(())
    }

    pub fn load_bm25(&self) -> crate::Result<BM25Index> {
        let path = self.index_dir.join("bm25.bin");
        let bytes = std::fs::read(&path)?;
        let index: BM25Index = bincode::deserialize(&bytes)
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        Ok(index)
    }

    // -----------------------------------------------------------------------
    // HNSW graph (bincode — includes flat embeddings)
    // -----------------------------------------------------------------------

    pub fn save_hnsw(&self, index: &HnswIndex) -> crate::Result<()> {
        let path = self.index_dir.join("hnsw.bin");
        let bytes = bincode::serialize(index)
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        std::fs::write(&path, bytes)?;
        Ok(())
    }

    pub fn load_hnsw(&self) -> crate::Result<HnswIndex> {
        let path = self.index_dir.join("hnsw.bin");
        let bytes = std::fs::read(&path)?;
        let index: HnswIndex = bincode::deserialize(&bytes)
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        Ok(index)
    }

    // -----------------------------------------------------------------------
    // File map: path → [chunk_id]  (for incremental update)
    // -----------------------------------------------------------------------

    pub fn save_filemap(&self, map: &HashMap<String, Vec<u64>>) -> crate::Result<()> {
        let path = self.index_dir.join("filemap.json");
        let json = serde_json::to_string(map)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load_filemap(&self) -> crate::Result<HashMap<String, Vec<u64>>> {
        let path = self.index_dir.join("filemap.json");
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, Vec<u64>> = serde_json::from_str(&content)?;
        Ok(map)
    }

    // -----------------------------------------------------------------------
    // File hashes (xxhash3): path → hash (for incremental update)
    // -----------------------------------------------------------------------

    pub fn save_file_hashes(&self, map: &HashMap<String, u64>) -> crate::Result<()> {
        let path = self.index_dir.join("file_hashes.json");
        let json = serde_json::to_string(map)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load_file_hashes(&self) -> crate::Result<HashMap<String, u64>> {
        let path = self.index_dir.join("file_hashes.json");
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, u64> = serde_json::from_str(&content)?;
        Ok(map)
    }

    // -----------------------------------------------------------------------
    // Knowledge graph
    // -----------------------------------------------------------------------

    pub fn save_graph(&self, graph: &CodeGraph) -> crate::Result<()> {
        let path = self.index_dir.join("graph.json");
        let json = serde_json::to_string(graph)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load_graph(&self) -> crate::Result<CodeGraph> {
        let path = self.index_dir.join("graph.json");
        if !path.exists() {
            return Ok(CodeGraph::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let graph: CodeGraph = serde_json::from_str(&content)?;
        Ok(graph)
    }

    // -----------------------------------------------------------------------
    // Policy Store (.needle/policy/<policy_id>.json)
    // -----------------------------------------------------------------------

    pub fn default_policy_dir() -> PathBuf {
        let root = Self::find_git_root()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        root.join(".needle").join("policy")
    }

    pub fn policy_dir(&self) -> PathBuf {
        if let Some(parent) = self.index_dir.parent() {
            parent.join("policy")
        } else {
            Self::default_policy_dir()
        }
    }

    pub fn save_policy(&self, doc: &crate::policy::clause::PolicyDocument) -> crate::Result<()> {
        let dir = self.policy_dir();
        std::fs::create_dir_all(&dir)?;
        let file_name = format!("{}.json", doc.id);
        let path = dir.join(file_name);
        let json = serde_json::to_string_pretty(doc)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load_policy(&self, id: &str) -> crate::Result<crate::policy::clause::PolicyDocument> {
        let dir = self.policy_dir();
        let file_name = if id.ends_with(".json") {
            id.to_string()
        } else {
            format!("{}.json", id)
        };
        let path = dir.join(&file_name);
        if !path.exists() {
            return Err(crate::error::Error::PolicyError(format!(
                "Policy with ID '{}' not found at '{}'",
                id,
                path.display()
            )));
        }
        let content = std::fs::read_to_string(&path)?;
        let doc: crate::policy::clause::PolicyDocument = serde_json::from_str(&content)?;
        Ok(doc)
    }

    pub fn list_policies(&self) -> crate::Result<Vec<crate::policy::clause::PolicyDocument>> {
        let dir = self.policy_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut policies = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(doc) = serde_json::from_str::<crate::policy::clause::PolicyDocument>(&content) {
                        policies.push(doc);
                    }
                }
            }
        }
        policies.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(policies)
    }

    // -----------------------------------------------------------------------
    // Disk size helpers
    // -----------------------------------------------------------------------

    pub fn index_size_bytes(&self) -> u64 {
        let files = [
            "chunks.json",
            "bm25.bin",
            "hnsw.bin",
            "filemap.json",
            "meta.json",
        ];
        files
            .iter()
            .map(|f| {
                std::fs::metadata(self.index_dir.join(f))
                    .map(|m| m.len())
                    .unwrap_or(0)
            })
            .sum()
    }

    pub fn file_size_bytes(&self, name: &str) -> u64 {
        std::fs::metadata(self.index_dir.join(name))
            .map(|m| m.len())
            .unwrap_or(0)
    }
}

pub fn human_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1e9)
    } else if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1e6)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1e3)
    } else {
        format!("{} B", bytes)
    }
}
