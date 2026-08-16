//! `needle serve` — local web UI backed by axum.

#[cfg(feature = "cloud")]
mod handlers_core;
#[cfg(feature = "cloud")]
mod handlers_import;
#[cfg(feature = "cloud")]
mod handlers_sentinel;

#[cfg(feature = "cloud")]
use axum::{
    http::HeaderMap,
    response::Html,
    routing::{get, post},
    Json, Router,
};
#[cfg(feature = "cloud")]
use tower_cookies::{CookieManagerLayer, Cookies};
#[cfg(feature = "cloud")]
use colored::Colorize;
#[cfg(feature = "cloud")]
use needle::{
    embedding::EmbeddingModel,
    graph::CodeGraph,
    query::QueryEngine,
    storage::Storage,
};
#[cfg(feature = "cloud")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "cloud")]
use needle::server::{indexer, oauth, users};
#[cfg(feature = "cloud")]
use std::collections::HashMap;
#[cfg(feature = "cloud")]
use tokio::sync::RwLock;
#[cfg(feature = "cloud")]
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

#[cfg(feature = "cloud")]
pub(super) struct UserEngine {
    pub(super) engine: Arc<Mutex<QueryEngine>>,
}

#[cfg(feature = "cloud")]
pub(super) struct ImportedIndex {
    pub(super) engine: Arc<Mutex<QueryEngine>>,
    pub(super) graph:  Arc<CodeGraph>,
}

#[cfg(feature = "cloud")]
#[derive(Clone, Serialize, Default)]
pub(super) struct ImportStatus {
    pub(super) phase:     String,
    pub(super) progress:  f32,
    pub(super) message:   String,
    pub(super) repo_url:  String,
    pub(super) repo_name: String,
    pub(super) files:     usize,
    pub(super) chunks:    usize,
    pub(super) error:     Option<String>,
}

#[cfg(feature = "cloud")]
#[derive(Clone)]
pub(super) struct AppState {
    pub(super) engine:        Option<Arc<Mutex<QueryEngine>>>,
    pub(super) storage:       Option<Storage>,
    pub(super) graph:         Arc<CodeGraph>,
    pub(super) has_ollama:    bool,
    pub(super) user_engines:  Arc<RwLock<HashMap<String, UserEngine>>>,
    pub(super) indexes_dir:   Arc<std::path::PathBuf>,
    pub(super) imported:      Arc<RwLock<Option<ImportedIndex>>>,
    pub(super) import_status: Arc<Mutex<ImportStatus>>,
    pub(super) tx:            tokio::sync::broadcast::Sender<String>,
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[cfg(feature = "cloud")]
#[derive(Deserialize)]
pub(super) struct SearchParams {
    pub(super) q: Option<String>,
    pub(super) lang: Option<String>,
    pub(super) limit: Option<usize>,
}

#[cfg(feature = "cloud")]
#[derive(Serialize)]
pub(super) struct SearchResponse {
    pub(super) results: Vec<ResultJson>,
    pub(super) timing: TimingJson,
    pub(super) total: usize,
}

#[cfg(feature = "cloud")]
#[derive(Serialize, Clone)]
pub(super) struct ResultJson {
    pub(super) chunk_id: u64,
    pub(super) file_path: String,
    pub(super) line_start: u32,
    pub(super) line_end: u32,
    pub(super) language: String,
    pub(super) chunk_type: String,
    pub(super) content: String,
    pub(super) score: f32,
    pub(super) signal: String,
}

#[cfg(feature = "cloud")]
#[derive(Serialize)]
pub(super) struct TimingJson {
    pub(super) total_ms: f64,
    pub(super) bm25_ms: f64,
    pub(super) hnsw_ms: f64,
    pub(super) embed_ms: f64,
    pub(super) fusion_ms: f64,
}

#[cfg(feature = "cloud")]
#[derive(Serialize)]
pub(super) struct StatusResponse {
    pub(super) total_chunks: u64,
    pub(super) total_files: u64,
    pub(super) watched_dirs: Vec<String>,
    pub(super) last_update_ts: u64,
    pub(super) embedding_model: String,
    pub(super) embedding_dim: u32,
    pub(super) vocabulary: u32,
    pub(super) disk_bytes: u64,
    pub(super) languages: Vec<String>,
    pub(super) has_ollama: bool,
    pub(super) is_cloud: bool,
    pub(super) has_index: bool,
}

#[cfg(feature = "cloud")]
#[derive(Deserialize)]
pub(super) struct OpenRequest {
    pub(super) path: String,
    pub(super) line: Option<u32>,
}

#[cfg(feature = "cloud")]
#[derive(Deserialize)]
pub(super) struct AskRequest {
    pub(super) question: String,
}

#[cfg(feature = "cloud")]
#[derive(Serialize)]
pub(super) struct AskResponse {
    pub(super) answer: String,
    pub(super) sources: Vec<ResultJson>,
    pub(super) model_used: String,
    pub(super) error: Option<String>,
}

#[cfg(feature = "cloud")]
#[derive(Deserialize)]
pub(super) struct SimilarRequest {
    pub(super) code: String,
    pub(super) exclude_id: Option<u64>,
    pub(super) limit: Option<usize>,
}

// Free-tier repo limit. Raise when paid plans exist.
#[cfg(feature = "cloud")]
pub(super) const MAX_REPOS_PER_USER: i64 = 3;

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[cfg(feature = "cloud")]
pub async fn run(port: u16, no_open: bool) -> needle::Result<()> {
    let bind_port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(port);

    let is_cloud = std::env::var("RAILWAY_ENVIRONMENT").is_ok()
        || std::env::var("RENDER").is_ok();

    let bind_addr = if is_cloud {
        format!("0.0.0.0:{}", bind_port)
    } else {
        format!("127.0.0.1:{}", bind_port)
    };

    let data_dir = std::path::PathBuf::from(
        std::env::var("DATA_DIR").unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".needle")
                .to_string_lossy()
                .to_string()
        })
    );

    let has_index = Storage::index_exists();

    let state = if has_index {
        let storage = Storage::new(Storage::default_index_dir())?;
        let config  = Storage::load_config().unwrap_or_default();
        let meta    = storage.load_metadata().unwrap_or_default();
        
        match (storage.load_bm25(), storage.load_hnsw(), storage.load_chunks()) {
            (Ok(bm25), Ok(hnsw), Ok(chunks)) => {
                let embedding = EmbeddingModel::from_metadata(&meta.embedding_model, meta.embedding_dim as usize)?;
                let graph    = storage.load_graph().unwrap_or_default();
                let has_ollama = embedding.is_ollama();
                let mut engine = QueryEngine::new(bm25, hnsw, chunks, embedding);
                engine.ef_search = config.hnsw_ef_search as usize;
                engine.rrf_k     = config.rrf_k;
                
                AppState {
                    engine:        Some(Arc::new(Mutex::new(engine))),
                    storage:       Some(storage),
                    graph:         Arc::new(graph),
                    has_ollama,
                    user_engines:  Arc::new(RwLock::new(HashMap::new())),
                    indexes_dir:   Arc::new(data_dir.join("indexes")),
                    imported:      Arc::new(RwLock::new(None)),
                    import_status: Arc::new(Mutex::new(ImportStatus::default())),
                    tx:            tokio::sync::broadcast::channel(100).0,
                }
            }
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                eprintln!("{} Index appears corrupted ({e}). Falling back to empty state. Run 'needle reindex'.", "⚠".yellow().bold());
                AppState {
                    engine:        None,
                    storage:       None,
                    graph:         Arc::new(CodeGraph::default()),
                    has_ollama:    false,
                    user_engines:  Arc::new(RwLock::new(HashMap::new())),
                    indexes_dir:   Arc::new(data_dir.join("indexes")),
                    imported:      Arc::new(RwLock::new(None)),
                    import_status: Arc::new(Mutex::new(ImportStatus::default())),
                    tx:            tokio::sync::broadcast::channel(100).0,
                }
            }
        }
    } else {
        if !is_cloud {
            eprintln!("{} No index found — running in marketing mode.\n  Run: needle init <dirs...> to index your code.",
                "Note:".yellow().bold());
        }
        AppState {
            engine:        None,
            storage:       None,
            graph:         Arc::new(CodeGraph::default()),
            has_ollama:    false,
            user_engines:  Arc::new(RwLock::new(HashMap::new())),
            indexes_dir:   Arc::new(data_dir.join("indexes")),
            imported:      Arc::new(RwLock::new(None)),
            import_status: Arc::new(Mutex::new(ImportStatus::default())),
            tx:            tokio::sync::broadcast::channel(100).0,
        }
    };

    // Background indexer (picks up pending cloud repos every 30s)
    {
        let cfg = Arc::new(indexer::IndexerConfig { data_dir: data_dir.clone() });
        tokio::spawn(indexer::run(cfg));
    }

    // SSE Watcher for index updates
    {
        let tx = state.tx.clone();
        let index_dir = data_dir.join("indexes");
        tokio::spawn(async move {
            use notify::{Watcher, RecursiveMode, EventKind};
            let (notify_tx, mut notify_rx) = tokio::sync::mpsc::channel(10);
            let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        let _ = notify_tx.blocking_send(());
                    }
                }
            }).unwrap();
            
            if index_dir.exists() {
                let _ = watcher.watch(&index_dir, RecursiveMode::Recursive);
            }
            
            while let Some(_) = notify_rx.recv().await {
                // Debounce
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                // Drain any extra events
                while notify_rx.try_recv().is_ok() {}
                let _ = tx.send("index_updated".to_string());
            }
        });
    }

    let has_oauth = oauth::OAuthConfig::from_env().is_some();

    let mode_info = serde_json::json!({
        "is_cloud":  is_cloud,
        "has_oauth": has_oauth,
        "has_index": has_index,
        "max_repos": MAX_REPOS_PER_USER,
    });

    let mut app = Router::new()
        .route("/", get(serve_ui))
        .route("/api/mode", get({
            let info = mode_info.clone();
            move || async move { Json(info) }
        }))
        .route("/api/search",          get(handlers_core::api_search))
        .route("/api/status",          get(handlers_core::api_status_handler))
        .route("/api/events",          get(handlers_core::api_events))
        .route("/api/open",            post(handlers_core::api_open))
        .route("/api/ask",             post(handlers_core::api_ask))
        .route("/api/similar",         post(handlers_core::api_similar))
        .route("/api/todos",           get(handlers_core::api_todos))
        .route("/api/files",           get(handlers_core::api_files))
        .route("/api/graph",           get(handlers_core::api_graph))
        .route("/api/blast-radius",    get(handlers_core::api_blast_radius))
        .route("/api/health",          get(handlers_core::api_health))
        .route("/api/security",        get(handlers_core::api_security))
        .route("/api/patterns",        get(handlers_core::api_patterns))
        .route("/api/git/churn",       get(handlers_core::api_git_churn))
        .route("/api/import/github",   post(handlers_import::api_import_github))
        .route("/api/import/local",    post(handlers_import::api_import_local))
        .route("/api/import/status",   get(handlers_import::api_import_status))
        .route("/api/import/clear",    post(handlers_import::api_import_clear))
        .route("/api/audit",           get(handlers_sentinel::api_sentinel_audit))
        .route("/api/ledger",          get(handlers_sentinel::api_sentinel_ledger))
        .route("/api/ledger/verify",   get(handlers_sentinel::api_sentinel_ledger_verify))
        .route("/api/ledger/sign",     post(handlers_sentinel::api_sentinel_ledger_sign))
        .route("/api/doctor",          get(handlers_sentinel::api_sentinel_doctor))
        .route("/auth/github",         get(handlers_import::api_auth_github))
        .route("/auth/callback",       get(handlers_import::api_auth_callback))
        .route("/auth/logout",         get(handlers_import::api_auth_logout))
        .route("/api/me",              get(handlers_import::api_me))
        .route("/api/me/regenerate-key", post(handlers_import::api_regenerate_key))
        .route("/api/me/revoke-key",     post(handlers_import::api_revoke_key))
        .route("/api/validate-key",      post(handlers_import::api_validate_key))
        .route("/api/github/repos",      get(handlers_import::api_github_repos_handler))
        .route("/api/repos",             get(handlers_import::api_repos))
        .route("/api/repos/connect",     post(handlers_import::api_repo_connect))
        .with_state(state)
        .layer(CookieManagerLayer::new());

    if let Some(cors) = build_cors() {
        app = app.layer(cors);
    }

    let listener = tokio::net::TcpListener::bind(&bind_addr).await.map_err(|e| {
        needle::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    })?;

    let url = format!("http://localhost:{}", bind_port);
    println!();
    println!("  {}  {}", "Needle".bold(), url.cyan().bold());
    if has_index { println!("  {} Index loaded", "✓".green()); }
    else { println!("  {} No index — marketing mode (run 'needle init' to index code)", "·".dimmed()); }
    if has_oauth { println!("  {} GitHub OAuth configured", "✓".green()); }
    println!("  {} Ctrl+C to stop\n", "·".dimmed());

    if !no_open && !is_cloud {
        let _ = open::that(&url);
    }

    axum::serve(listener, app).await.map_err(|e| {
        needle::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    })?;

    Ok(())
}

#[cfg(feature = "cloud")]
fn build_cors() -> Option<tower_http::cors::CorsLayer> {
    Some(tower_http::cors::CorsLayer::permissive())
}

#[cfg(feature = "cloud")]
async fn serve_ui() -> Html<&'static str> {
    Html(include_str!("../../assets/ui.html"))
}

#[cfg(feature = "cloud")]
pub(super) async fn resolve_user(cookies: &Cookies, headers: &HeaderMap) -> Option<users::User> {
    if let Some(u) = oauth::current_user_from_cookies(cookies).await {
        return Some(u);
    }
    let auth = headers.get("authorization")?.to_str().ok()?;
    let key  = auth.strip_prefix("Bearer ")?;
    let pool = users::pool().await.ok()?;
    users::get_user_by_api_key(pool, key).await.ok().flatten()
}

#[cfg(feature = "cloud")]
pub(super) async fn load_cloud_engines(
    state: &AppState,
    user: Option<&users::User>,
) -> Vec<Arc<Mutex<QueryEngine>>> {
    let mut out = vec![];
    let Some(user) = user else { return out };
    let Ok(pool)   = users::pool().await else { return out };
    let Ok(repos)  = users::list_user_repos(pool, &user.id).await else { return out };

    for repo in repos.into_iter().filter(|r| r.status == "indexed") {
        let safe_name  = repo.repo_full.replace('/', "_");
        let engine_key = format!("{}:{}", user.id, safe_name);

        let cached = state.user_engines.read().await.get(&engine_key).map(|ue| ue.engine.clone());

        let engine = if let Some(e) = cached {
            e
        } else {
            let idx_dir = state.indexes_dir.join(&user.id).join(&safe_name).join("index");
            if !idx_dir.exists() { continue; }
            match tokio::task::spawn_blocking(move || -> Result<QueryEngine, String> {
                let s  = Storage::new(idx_dir).map_err(|e| e.to_string())?;
                let m  = s.load_metadata().unwrap_or_default();
                let b  = s.load_bm25().map_err(|e| e.to_string())?;
                let h  = s.load_hnsw().map_err(|e| e.to_string())?;
                let ch = s.load_chunks().map_err(|e| e.to_string())?;
                let em = EmbeddingModel::from_metadata(&m.embedding_model, m.embedding_dim as usize)
                    .map_err(|e| e.to_string())?;
                Ok(QueryEngine::new(b, h, ch, em))
            }).await {
                Ok(Ok(qe)) => {
                    let arc = Arc::new(Mutex::new(qe));
                    state.user_engines.write().await.insert(engine_key, UserEngine { engine: arc.clone() });
                    arc
                }
                _ => continue,
            }
        };

        out.push(engine);
    }
    out
}

#[cfg(not(feature = "cloud"))]
pub async fn run(_port: u16, _no_open: bool) -> needle::Result<()> {
    println!("needle serve is disabled in sovereign build mode. Use CLI commands (needle search, needle audit, needle doctor) or stdio MCP.");
    Ok(())
}
