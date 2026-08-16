use std::path::PathBuf;
use std::process::Child;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

static SERVER: Mutex<Option<Child>> = Mutex::new(None);

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            cmd_open_folder,
            cmd_index_folder,
            cmd_get_indexed_dirs,
            cmd_get_graph,
            cmd_search,
            cmd_ask,
        ])
        .setup(|app| {
            // Axum server is no longer needed in the native desktop build!

            // Build system-tray menu
            let open_item   = MenuItem::with_id(app, "open",   "Open Needle",   true, None::<&str>)?;
            let folder_item = MenuItem::with_id(app, "folder", "Index a folder…", true, None::<&str>)?;
            let quit_item   = MenuItem::with_id(app, "quit",   "Quit",           true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &folder_item, &quit_item])?;

            TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open"   => show_or_create_window(app),
                    "folder" => trigger_folder_pick(app),
                    "quit"   => {
                        kill_server();
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
                        show_or_create_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // The main window is now created automatically by tauri.conf.json
            Ok(())
        })
        .on_window_event(|window, event| {
            // Minimize to tray instead of closing
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("Error running Needle desktop app");
}

// ---------------------------------------------------------------------------
// Tauri commands (callable from the frontend via invoke())
// ---------------------------------------------------------------------------

/// Opens a native folder-picker dialog and returns the selected path.
#[tauri::command]
async fn cmd_open_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |path| {
        let _ = tx.send(path);
    });
    let path = rx.await.map_err(|e| e.to_string())?;
    Ok(path.map(|p| p.to_string()))
}

/// Runs `needle init <dir>` in the background, then reloads the web UI.
/// Returns immediately; indexing is async (progress visible in the UI).
#[tauri::command]
async fn cmd_index_folder(app: tauri::AppHandle, dir: String) -> Result<(), String> {
    let needle_bin = resolve_needle_binary(&app);
    std::process::Command::new(&needle_bin)
        .args(["init", &dir])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn cmd_get_indexed_dirs() -> Result<Vec<String>, String> {
    Ok(vec![]) // deprecated
}

/// Returns the entire CodeGraph serialized as JSON
#[tauri::command]
async fn cmd_get_graph() -> Result<String, String> {
    let index_dir = needle::storage::Storage::default_index_dir();
    let storage = needle::storage::Storage::new(index_dir)
        .map_err(|e| format!("Storage error: {}", e))?;
    let graph = storage.load_graph()
        .map_err(|e| format!("Failed to load graph: {}", e))?;
    
    // We map graph.nodes and graph.edges to JSON
    // The frontend expects { nodes, links } where links have { source, target, value }
    
    let mut nodes = Vec::new();
    let god_nodes = needle::graph::compute_god_nodes(&graph, 50); // Get top 50 god nodes
    let communities = needle::graph::compute_communities(&graph); // Get communities
    
    // Convert to quick lookup maps
    let mut god_node_degrees = std::collections::HashMap::new();
    for (node_id, degree) in god_nodes {
        god_node_degrees.insert(node_id, degree);
    }
    
    for (i, node) in graph.nodes.iter().enumerate() {
        let node_id = i as u32;
        let degree = god_node_degrees.get(&node_id).copied().unwrap_or(1);
        let group = communities.get(&node_id).copied().unwrap_or(0);
        
        let radius = 5 + (degree as f32).sqrt() as i32 * 3; // Scale radius by connections
        
        nodes.push(serde_json::json!({
            "id": i,
            "name": node.name,
            "path": node.file_path,
            "kind": format!("{:?}", node.kind),
            "group": group,
            "radius": radius
        }));
    }
    
    let mut links = Vec::new();
    for edge in &graph.edges {
        links.push(serde_json::json!({
            "source": edge.from,
            "target": edge.to,
            "value": 1
        }));
    }
    
    let out = serde_json::json!({
        "nodes": nodes,
        "links": links
    });
    
    Ok(serde_json::to_string(&out).unwrap())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Emit a request to the frontend to open the folder picker.
/// Used from the tray menu "Index a folder…" item.
fn trigger_folder_pick(app: &tauri::AppHandle) {
    show_or_create_window(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("trigger-folder-pick", ());
    }
}

fn create_main_window(handle: &tauri::AppHandle) {
    WebviewWindowBuilder::new(
        handle,
        "main",
        WebviewUrl::App("index.html".into()),
    )
    .title("Sentinel Auditor (Air-Gapped)")
    .inner_size(1400.0, 900.0)
    .min_inner_size(900.0, 600.0)
    .center()
    .visible(true)
    .build()
    .expect("Failed to create Needle window");
}

fn show_or_create_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        create_main_window(app);
    }
}

fn kill_server() {
    if let Some(mut child) = SERVER.lock().unwrap().take() {
        let _ = child.kill();
    }
}

fn resolve_needle_binary(handle: &tauri::AppHandle) -> PathBuf {
    // Production: binary lives next to the app in a resources/ dir
    #[cfg(not(debug_assertions))]
    if let Ok(resource_dir) = handle.path().resource_dir() {
        let candidate = resource_dir.join(if cfg!(windows) {
            "needle.exe"
        } else {
            "needle"
        });
        if candidate.exists() {
            return candidate;
        }
    }

    // Development: use the workspace's release binary
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    workspace_root.join("target").join("release").join(if cfg!(windows) {
        "needle.exe"
    } else {
        "needle"
    })
}

#[tauri::command]
async fn cmd_search(q: String, limit: usize) -> Result<String, String> {
    let index_dir = needle::storage::Storage::default_index_dir();
    let storage = needle::storage::Storage::new(index_dir)
        .map_err(|e| format!("Storage error: {}", e))?;
    let config = needle::storage::Storage::load_config().unwrap_or_default();
    
    let bm25 = storage.load_bm25().map_err(|e| format!("BM25 error: {}", e))?;
    let hnsw = storage.load_hnsw().map_err(|e| format!("HNSW error: {}", e))?;
    let chunks = storage.load_chunks().map_err(|e| format!("Chunks error: {}", e))?;
    let embedding = needle::embedding::EmbeddingModel::new(config.embedding_dim).map_err(|e| format!("Embedding error: {}", e))?;

    let mut engine = needle::query::QueryEngine::new(bm25, hnsw, chunks, embedding);
    
    let (results, _) = engine.search(&q, limit, None)
        .map_err(|e| format!("Search error: {}", e))?;
        
    let mut out = Vec::new();
    for r in results {
        out.push(serde_json::json!({
            "file_path": r.file_path.replace('\\', "/"),
            "line_start": r.line_start,
            "line_end": r.line_end,
            "language": r.language.short_name(),
            "content": r.content,
            "score": r.score
        }));
    }
    
    Ok(serde_json::to_string(&out).unwrap())
}

#[tauri::command]
async fn cmd_ask(question: String) -> Result<String, String> {
    // For sovereign mode, we only use the local Ollama instance without sending code to the cloud.
    let mut llm = needle::llm::LlmClient::from_env();
    
    let response = llm.complete("You are a helpful coding assistant. Use the provided context to answer the user's question.", &question)
        .await
        .map_err(|e| format!("Ollama failed: {}", e))?;
        
    Ok(response)
}
