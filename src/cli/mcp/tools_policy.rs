//! MCP tools for policy compliance (M3).
//!
//! Exposes 4 new tools: get_obligations, check_compliance, get_policy_gaps, get_compliance_report.

use needle::policy::linker::{link_document, ComplianceStatus};
use needle::query::QueryEngine;
use needle::storage::Storage;
use serde_json::Value;
use std::collections::HashMap;

type ToolResult = Result<String, String>;

/// Helper: load the query engine from the local index.
fn load_engine() -> Result<QueryEngine, String> {
    let storage = Storage::new(Storage::default_index_dir())
        .map_err(|e| format!("Storage error: {e}"))?;
    let chunks: HashMap<_, _> = storage.load_chunks().map_err(|e| format!("Load chunks: {e}"))?;
    let bm25 = storage.load_bm25().map_err(|e| format!("Load BM25: {e}"))?;
    let hnsw = storage.load_hnsw().map_err(|e| format!("Load HNSW: {e}"))?;
    let embedding = needle::embedding::EmbeddingModel::new(384)
        .map_err(|e| format!("Embedding init error: {e}"))?;
    Ok(QueryEngine::new(bm25, hnsw, chunks, embedding))
}

/// Returns all ingested policy documents and their obligations.
///
/// Optional `doc` filter to return only a specific policy by id or name.
pub fn get_obligations(args: &Value) -> ToolResult {
    let doc_filter = args["doc"].as_str();
    let storage = Storage::new(Storage::default_index_dir())
        .map_err(|e| format!("Storage error: {e}"))?;
    let policies = storage.list_policies().map_err(|e| format!("{e}"))?;

    if policies.is_empty() {
        return Ok("No policies ingested. Run: sentinel policy ingest <file>".to_string());
    }

    let mut out = String::from("## Ingested Policy Obligations\n\n");
    for doc in &policies {
        if let Some(filter) = doc_filter {
            if !doc.id.contains(filter) && !doc.name.to_lowercase().contains(&filter.to_lowercase()) {
                continue;
            }
        }
        out.push_str(&format!("### {} (v{})\nID: `{}`\n\n", doc.name, doc.version, doc.id));
        for clause in &doc.clauses {
            for obl in &clause.obligations {
                out.push_str(&format!(
                    "- **[{}]** `{}` [{}] [{}] {}\n",
                    clause.clause_number,
                    obl.id,
                    obl.obligation_type.as_str(),
                    obl.severity.as_str(),
                    obl.title
                ));
            }
        }
        out.push('\n');
    }
    Ok(out)
}

/// Runs the compliance linker for a specific obligation by `obligation_id`.
pub fn check_compliance(args: &Value) -> ToolResult {
    let obligation_id = args["obligation_id"]
        .as_str()
        .ok_or("obligation_id is required")?;

    let storage = Storage::new(Storage::default_index_dir())
        .map_err(|e| format!("Storage error: {e}"))?;
    let policies = storage.list_policies().map_err(|e| format!("{e}"))?;
    let engine = load_engine()?;

    for doc in &policies {
        for clause in &doc.clauses {
            if let Some(obl) = clause.obligations.iter().find(|o| o.id == obligation_id) {
                let link = needle::policy::linker::link_obligation_pub(clause, obl, &engine)
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_string_pretty(&link).unwrap_or_else(|_| format!("{link:?}")));
            }
        }
    }

    Err(format!("Obligation '{obligation_id}' not found in any ingested policy."))
}

/// Returns all obligations with `NoImplementationFound` status — the compliance gaps.
pub fn get_policy_gaps(args: &Value) -> ToolResult {
    let doc_filter = args["doc"].as_str();
    let storage = Storage::new(Storage::default_index_dir())
        .map_err(|e| format!("Storage error: {e}"))?;
    let policies = storage.list_policies().map_err(|e| format!("{e}"))?;
    let engine = load_engine()?;

    let mut out = String::from("## Policy Compliance Gaps\n\nObligations with no code implementation found:\n\n");
    let mut total_gaps = 0usize;

    for doc in &policies {
        if let Some(filter) = doc_filter {
            if !doc.id.contains(filter) && !doc.name.to_lowercase().contains(&filter.to_lowercase()) {
                continue;
            }
        }
        let report = link_document(doc, &engine).map_err(|e| format!("{e}"))?;
        for link in report.links.iter().filter(|l| matches!(&l.status, ComplianceStatus::NoImplementationFound)) {
            out.push_str(&format!(
                "- **[{}]** {} — `{}` [{}]\n",
                link.clause_number, link.title, link.severity, link.obligation_type
            ));
            total_gaps += 1;
        }
    }

    if total_gaps == 0 {
        out.push_str("✅ No gaps found — all obligations have code evidence.\n");
    } else {
        out.push_str(&format!("\n**Total gaps: {total_gaps}**\n"));
    }
    Ok(out)
}

/// Runs full compliance audit across all ingested policies and returns a Markdown report.
pub fn get_compliance_report(args: &Value) -> ToolResult {
    let json_out = args["format"].as_str() == Some("json");
    let storage = Storage::new(Storage::default_index_dir())
        .map_err(|e| format!("Storage error: {e}"))?;
    let policies = storage.list_policies().map_err(|e| format!("{e}"))?;
    if policies.is_empty() {
        return Ok("No policies ingested. Run: sentinel policy ingest <file>".to_string());
    }
    let engine = load_engine()?;

    let mut all_reports = Vec::new();
    let mut total_satisfied = 0usize;
    let mut total_violated = 0usize;
    let mut total_no_evidence = 0usize;

    for doc in &policies {
        let report = link_document(doc, &engine).map_err(|e| format!("{e}"))?;
        total_satisfied += report.satisfied;
        total_violated += report.violated;
        total_no_evidence += report.no_evidence;
        all_reports.push(report);
    }

    if json_out {
        return Ok(serde_json::to_string_pretty(&all_reports).unwrap_or_default());
    }

    let mut md = String::from("# NEEDLE-SENTINEL Compliance Report\n\n");
    md.push_str(&format!("| ✅ Satisfied | ❌ Violated | ⚠️ No Evidence | Total |\n|---|---|---|---|\n"));
    md.push_str(&format!("| {} | {} | {} | {} |\n\n",
        total_satisfied, total_violated, total_no_evidence,
        total_satisfied + total_violated + total_no_evidence));

    for report in &all_reports {
        let score_pct = (report.compliance_score() * 100.0) as u32;
        md.push_str(&format!("---\n\n## {} (v{}) — **{}% compliant**\n\n",
            report.policy_name, report.policy_version, score_pct));

        for link in &report.links {
            let status_icon = match &link.status {
                ComplianceStatus::Satisfied { .. } => "✅",
                ComplianceStatus::Violated { .. } => "❌",
                ComplianceStatus::NoImplementationFound => "⚠️",
            };
            md.push_str(&format!("{status_icon} **[{}]** {} `[{}]`\n", link.clause_number, link.title, link.severity));
            match &link.status {
                ComplianceStatus::Violated { reason, .. } => {
                    md.push_str(&format!("   > ⚡ Violation: {reason}\n"));
                }
                ComplianceStatus::Satisfied { evidence } => {
                    if let Some(ev) = evidence.first() {
                        md.push_str(&format!("   > Evidence: `{}:{}`\n", ev.file_path, ev.line_start));
                    }
                }
                ComplianceStatus::NoImplementationFound => {}
            }
        }
        md.push('\n');
    }
    Ok(md)
}
