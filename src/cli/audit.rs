//! `sentinel audit` — run the full compliance linker and print a report.
//!
//! Loads every ingested policy document from `.needle/policy/`,
//! runs the compliance linker against the current code index, and prints a
//! Markdown report (mirroring the existing `needle report` command style).
//!
//! Use `--ledger` to auto-append the report to the cryptographic audit ledger.

use colored::Colorize;
use needle::embedding::EmbeddingModel;
use needle::indexing::{bm25::BM25Index, hnsw::HnswIndex};
use needle::policy::linker::{ComplianceStatus, link_document};
use needle::query::QueryEngine;
use needle::storage::Storage;
use needle::Result;
use std::collections::HashMap;

pub async fn run(ledger: bool, output: Option<String>, json: bool, strict: bool, pdf: Option<String>) -> Result<()> {
    let storage = Storage::new(Storage::default_index_dir())?;

    // ── Load code index ───────────────────────────────────────────────────────
    if !Storage::index_exists() {
        eprintln!(
            "{}",
            "No index found. Run: sentinel init <dirs...>".red()
        );
        return Err(needle::error::Error::IndexNotFound(
            "No index found. Run: sentinel init <dirs...>".to_string()
        ));
    }

    let chunks: HashMap<u64, needle::schema::Chunk> = storage.load_chunks()?;
    let bm25: BM25Index = storage.load_bm25()?;
    let hnsw: HnswIndex = storage.load_hnsw()?;
    let embedding = EmbeddingModel::new(384).map_err(|e| needle::error::Error::EmbeddingError(e.to_string()))?;
    let engine = QueryEngine::new(bm25, hnsw, chunks, embedding);

    // ── Load all ingested policy documents ────────────────────────────────────
    let policies = storage.list_policies()?;
    if policies.is_empty() {
        println!(
            "{}",
            "No policies ingested. Run: sentinel policy ingest <file>".yellow()
        );
        return Ok(());
    }

    println!("{}", format!("Running compliance audit across {} policy document(s)...\n", policies.len()).bold());

    let mut all_reports = Vec::new();
    let mut total_satisfied = 0usize;
    let mut total_violated = 0usize;
    let mut total_no_evidence = 0usize;

    for doc in &policies {
        let report = link_document(doc, &engine)?;
        total_satisfied += report.satisfied;
        total_violated += report.violated;
        total_no_evidence += report.no_evidence;
        all_reports.push(report);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&all_reports)?);
        return Ok(());
    }

    // ── Markdown report ───────────────────────────────────────────────────────
    let mut md = String::new();
    md.push_str("# NEEDLE-SENTINEL Compliance Audit Report\n\n");
    md.push_str(&format!("> Generated: {}\n\n", chrono::Utc::now().to_rfc3339()));
    md.push_str("## Summary\n\n");
    md.push_str(&format!("| Metric | Count |\n|---|---|\n"));
    md.push_str(&format!("| ✅ Satisfied | {} |\n", total_satisfied));
    md.push_str(&format!("| ❌ Violated  | {} |\n", total_violated));
    md.push_str(&format!("| ⚠️  No Evidence | {} |\n", total_no_evidence));
    md.push_str(&format!("| Total Obligations | {} |\n\n", total_satisfied + total_violated + total_no_evidence));

    for report in &all_reports {
        let score_pct = (report.compliance_score() * 100.0) as u32;
        md.push_str(&format!("---\n\n## Policy: {} (v{})\n\n", report.policy_name, report.policy_version));
        md.push_str(&format!("**Compliance Score: {}%** ({} satisfied / {} total)\n\n",
            score_pct, report.satisfied, report.total_obligations));

        if report.violated > 0 {
            md.push_str("### ❌ Violations\n\n");
            for link in report.links.iter().filter(|l| matches!(&l.status, ComplianceStatus::Violated { .. })) {
                md.push_str(&format!("- **[{}]** {} — `{}`\n", link.clause_number, link.title, link.severity));
                if let ComplianceStatus::Violated { reason, conflicting } = &link.status {
                    md.push_str(&format!("  - Reason: {reason}\n"));
                    for ev in conflicting.iter().take(2) {
                        md.push_str(&format!("  - Evidence: `{}:{}` \n", ev.file_path, ev.line_start));
                    }
                }
            }
            md.push('\n');
        }

        if report.no_evidence > 0 {
            md.push_str("### ⚠️ No Implementation Found\n\n");
            for link in report.links.iter().filter(|l| matches!(&l.status, ComplianceStatus::NoImplementationFound)) {
                md.push_str(&format!("- **[{}]** {} — `{}`\n", link.clause_number, link.title, link.obligation_type));
            }
            md.push('\n');
        }

        if report.satisfied > 0 {
            md.push_str("### ✅ Satisfied\n\n");
            for link in report.links.iter().filter(|l| matches!(&l.status, ComplianceStatus::Satisfied { .. })) {
                let top_file = if let ComplianceStatus::Satisfied { evidence } = &link.status {
                    evidence.first().map(|e| format!("{}:{}", e.file_path, e.line_start)).unwrap_or_default()
                } else { String::new() };
                md.push_str(&format!("- **[{}]** {} → `{}`\n", link.clause_number, link.title, top_file));
            }
            md.push('\n');
        }
    }

    // Write or print the report
    match &output {
        Some(path) => {
            std::fs::write(path, &md)?;
            println!("{}", format!("✓ Compliance report written to: {path}").green().bold());
        }
        None => print!("{md}"),
    }

    // ── Optional ledger append ────────────────────────────────────────────────
    if ledger {
        let report_path = output.as_deref().unwrap_or("audit_report.md");
        // Write to temp if no output path was set
        if output.is_none() {
            std::fs::write(report_path, &md)?;
        }
        use needle::ledger::{append_to_ledger, default_key_priv_path, default_ledger_path, EntryType, LedgerKeypair};
        use std::str::FromStr;
        let key_path = default_key_priv_path();
        if !key_path.exists() {
            eprintln!("{}", "⚠ No ledger key found. Run: sentinel ledger keygen".yellow());
        } else {
            let kp = LedgerKeypair::load_from_file(&key_path)?;
            let payload = serde_json::json!({
                "report_path": report_path,
                "policies": all_reports.iter().map(|r| &r.policy_id).collect::<Vec<_>>(),
                "total_satisfied": total_satisfied,
                "total_violated": total_violated,
                "total_no_evidence": total_no_evidence,
            });
            let block = append_to_ledger(&default_ledger_path(), &kp, EntryType::from_str("compliance_audit").unwrap(), payload)?;
            println!("{}", format!("✓ Appended to ledger as block #{}", block.sequence).green());
        }
    }

    // ── PDF Generation ────────────────────────────────────────────────────────
    if let Some(pdf_path) = pdf {
        println!("{}", format!("Generating PDF report: {pdf_path}...").cyan());
        
        // Convert MD to HTML using pulldown-cmark
        let parser = pulldown_cmark::Parser::new(&md);
        let mut html_output = String::new();
        html_output.push_str("<html><head><style>body { font-family: sans-serif; padding: 40px; } table { width: 100%; border-collapse: collapse; } th, td { border: 1px solid #ddd; padding: 8px; }</style></head><body>");
        pulldown_cmark::html::push_html(&mut html_output, parser);
        html_output.push_str("</body></html>");
        
        let tmp_html = std::env::temp_dir().join("sentinel_audit_tmp.html");
        std::fs::write(&tmp_html, html_output)?;
        
        // Call Edge to print to PDF
        let edge_path = r#"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"#;
        if std::path::Path::new(edge_path).exists() {
            let abs_pdf = std::path::Path::new(&pdf_path).canonicalize().unwrap_or_else(|_| std::env::current_dir().unwrap().join(&pdf_path));
            
            let status = std::process::Command::new(edge_path)
                .args(&[
                    "--headless",
                    "--disable-gpu",
                    "--run-all-compositor-stages-before-draw",
                    &format!("--print-to-pdf={}", abs_pdf.display()),
                    tmp_html.to_str().unwrap()
                ])
                .status();
                
            if status.is_ok() && status.unwrap().success() {
                println!("{}", format!("✓ PDF report successfully written to: {pdf_path}").green().bold());
            } else {
                eprintln!("{}", "⚠ Failed to generate PDF using Edge headless.".red());
            }
        } else {
            eprintln!("{}", "⚠ msedge.exe not found. Cannot generate PDF.".yellow());
        }
    }

    if strict && total_violated > 0 {
        return Err(needle::Error::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Strict Mode: {} violations found. Failing CI/CD pipeline.", total_violated),
        )));
    }

    Ok(())
}
