use axum::{Json, response::{IntoResponse}};
use serde_json::Value;
use std::collections::HashMap;

// No longer serving standalone sentinel

pub async fn api_sentinel_audit() -> impl IntoResponse {
    let json_val = match run_audit_logic().await {
        Ok(v) => v,
        Err(e) => serde_json::json!({"error": e}),
    };
    Json(json_val)
}

async fn run_audit_logic() -> Result<Value, String> {
    let storage = needle::storage::Storage::new(needle::storage::Storage::default_index_dir())
        .map_err(|e| e.to_string())?;
    
    let policies = storage.list_policies().map_err(|e| e.to_string())?;
    if policies.is_empty() {
        return Ok(serde_json::json!([]));
    }

    let chunks = storage.load_chunks().map_err(|e| e.to_string())?;
    let bm25 = storage.load_bm25().map_err(|e| e.to_string())?;
    let hnsw = storage.load_hnsw().map_err(|e| e.to_string())?;
    let embedding = needle::embedding::EmbeddingModel::new(384).map_err(|e| e.to_string())?;
    let engine = needle::query::QueryEngine::new(bm25, hnsw, chunks, embedding);

    let mut all_reports = Vec::new();
    for doc in &policies {
        let report = needle::policy::linker::link_document(doc, &engine).map_err(|e| e.to_string())?;
        all_reports.push(report);
    }
    
    Ok(serde_json::to_value(all_reports).unwrap())
}

pub async fn api_sentinel_ledger() -> impl IntoResponse {
    let mut blocks = Vec::new();
    if let Ok(content) = std::fs::read_to_string(&needle::ledger::default_ledger_path()) {
        for line in content.lines().filter(|l| !l.trim().is_empty()) {
            if let Ok(block) = serde_json::from_str::<serde_json::Value>(line) {
                blocks.push(block);
            }
        }
    }
    Json(serde_json::json!({
        "total_blocks": blocks.len(),
        "blocks": blocks,
    }))
}

pub async fn api_sentinel_doctor() -> impl IntoResponse {
    let report = crate::cli::doctor::run_diagnostics(false, false, "http://127.0.0.1:11434", None);
    Json(serde_json::json!(report))
}

pub async fn api_sentinel_ledger_verify() -> impl IntoResponse {
    match needle::ledger::verifier::verify_ledger_file(&needle::ledger::default_ledger_path()) {
        Ok(summary) => Json(serde_json::json!({"valid": summary.is_valid, "total_blocks": summary.total_blocks})),
        Err(e) => Json(serde_json::json!({"valid": false, "error": e.to_string()})),
    }
}

pub async fn api_sentinel_ledger_sign() -> impl IntoResponse {
    match run_audit_logic().await {
        Ok(audit_val) => {
            let key_path = needle::ledger::default_key_priv_path();
            if !key_path.exists() {
                return Json(serde_json::json!({"error": "No ledger key found. Run: needle ledger keygen"}));
            }
            if let Ok(kp) = needle::ledger::LedgerKeypair::load_from_file(&key_path) {
                if let Ok(block) = needle::ledger::append_to_ledger(
                    &needle::ledger::default_ledger_path(),
                    &kp,
                    std::str::FromStr::from_str("compliance_audit").unwrap(),
                    audit_val
                ) {
                    return Json(serde_json::json!({"sequence": block.sequence}));
                }
            }
            Json(serde_json::json!({"error": "Failed to sign"}))
        }
        Err(e) => Json(serde_json::json!({"error": e})),
    }
}
