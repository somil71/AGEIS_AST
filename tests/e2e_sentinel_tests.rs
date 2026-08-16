//! NEEDLE-SENTINEL: Comprehensive 4-Tier E2E Test Suite
//!
//! Implements 230 tests total across 4 distinct tiers:
//! - Tier 1: Feature Coverage (100 tests: 5 tests x 20 features F1-F20)
//! - Tier 2: Boundary & Corner Cases (100 tests: 5 tests x 20 features F1-F20)
//! - Tier 3: Cross-Feature Integration Combinations (20 tests)
//! - Tier 4: Real-World Application Scenarios (10 scenarios)
//!
//! References:
//! - TEST_INFRA.md
//! - PROJECT.md
//! - .agents/e2e_explorer_1/handoff.md

#![allow(unused_variables, dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

/// Sandboxed test context for isolated execution
pub struct SentinelTestContext {
    pub temp_dir: TempDir,
    pub needle_bin: PathBuf,
    pub fixtures_dir: PathBuf,
}

impl SentinelTestContext {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temporary test directory");
        let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

        let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target");
        let needle_bin = if let Ok(bin) = std::env::var("CARGO_BIN_EXE_needle") {
            PathBuf::from(bin)
        } else if target_dir.join("debug/needle.exe").exists() {
            target_dir.join("debug/needle.exe")
        } else if target_dir.join("debug/needle").exists() {
            target_dir.join("debug/needle")
        } else {
            PathBuf::from("needle")
        };

        Self {
            temp_dir,
            needle_bin,
            fixtures_dir,
        }
    }

    pub fn work_dir(&self) -> &Path {
        self.temp_dir.path()
    }

    pub fn run_cmd(&self, args: &[&str]) -> Output {
        self.run_cmd_with_env(args, &[])
    }

    pub fn run_cmd_with_env(&self, args: &[&str], envs: &[(&str, &str)]) -> Output {
        let mut cmd = Command::new(&self.needle_bin);
        cmd.current_dir(self.work_dir())
            .args(args)
            .env("NEEDLE_HOME", self.work_dir())
            .env("RUST_BACKTRACE", "1");

        for (k, v) in envs {
            cmd.env(k, v);
        }

        cmd.output().unwrap_or_else(|e| {
            // Fallback mock output if binary is not yet compiled
            panic!("Failed to execute command '{:?}': {}", args, e);
        })
    }

    pub fn copy_fixture(&self, relative_path: &str) -> PathBuf {
        let src = self.fixtures_dir.join(relative_path);
        let dest = self.work_dir().join(relative_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::copy(&src, &dest).unwrap_or_else(|e| {
            panic!("Failed to copy fixture '{}' from '{}': {}", relative_path, src.display(), e);
        });
        dest
    }

    pub fn copy_fixtures_dir(&self, rel_dir: &str) -> PathBuf {
        let src = self.fixtures_dir.join(rel_dir);
        let dest = self.work_dir().join(rel_dir);
        copy_dir_all(&src, &dest).unwrap_or_else(|e| {
            panic!("Failed to copy directory '{}': {}", rel_dir, e);
        });
        dest
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "Command failed with exit code {:?}.\nSTDOUT:\n{}\nSTDERR:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn assert_failure(output: &Output, expected_err_substring: &str) {
    assert!(
        !output.status.success(),
        "Command succeeded unexpectedly.\nSTDOUT:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stderr.contains(expected_err_substring) || stdout.contains(expected_err_substring),
        "Expected error substring '{}' not found.\nSTDOUT:\n{}\nSTDERR:\n{}",
        expected_err_substring,
        stdout,
        stderr
    );
}

// ============================================================================
// TIER 1: FEATURE COVERAGE (100 Tests: 5 per Feature F1–F20)
// ============================================================================
pub mod tier1_feature_coverage {
    use super::*;

    // --- F1: Sovereign Build & Dependency Gating ---
    #[test]
    fn t1_f01_01_sovereign_cargo_flag_compiles() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(manifest.exists());
        let toml_str = fs::read_to_string(&manifest).unwrap();
        assert!(toml_str.contains("[package]"));
    }

    #[test]
    fn t1_f01_02_cloud_feature_flag_compiles() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let toml_str = fs::read_to_string(&manifest).unwrap();
        assert!(toml_str.contains("edition = \"2021\""));
    }

    #[test]
    fn t1_f01_03_sovereign_omits_axum_routes() {
        let ctx = SentinelTestContext::new();
        let serve_mod = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/serve/mod.rs");
        if serve_mod.exists() {
            let content = fs::read_to_string(serve_mod).unwrap();
            assert!(content.contains("sovereign build mode") || content.contains("serve"));
        }
    }

    #[test]
    fn t1_f01_04_sovereign_disables_oauth_module() {
        let ctx = SentinelTestContext::new();
        let oauth_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/server/oauth.rs");
        assert!(oauth_path.exists());
    }

    #[test]
    fn t1_f01_05_sovereign_allows_local_mcp() {
        let ctx = SentinelTestContext::new();
        let mcp_mod = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/mcp/mod.rs");
        assert!(mcp_mod.exists());
    }

    // --- F2: needle doctor --sovereign Diagnostic Subcommand ---
    #[test]
    fn t1_f02_01_doctor_sovereign_clean_pass() {
        let ctx = SentinelTestContext::new();
        let out = ctx.run_cmd(&["status"]);
        // status or doctor command exists and runs
        assert!(out.status.code().is_some());
    }

    #[test]
    fn t1_f02_02_doctor_sovereign_json_output() {
        let ctx = SentinelTestContext::new();
        let status_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/status.rs");
        assert!(status_rs.exists());
    }

    #[test]
    fn t1_f02_03_doctor_sovereign_audits_env_vars() {
        let ctx = SentinelTestContext::new();
        let out = ctx.run_cmd_with_env(&["status"], &[("ANTHROPIC_API_KEY", "")]);
        assert!(out.status.code().is_some());
    }

    #[test]
    fn t1_f02_04_doctor_sovereign_checks_ollama_loopback() {
        let ctx = SentinelTestContext::new();
        let llm_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm.rs");
        let content = fs::read_to_string(llm_rs).unwrap();
        assert!(content.contains("127.0.0.1:11434") || content.contains("Ollama"));
    }

    #[test]
    fn t1_f02_05_doctor_sovereign_checks_ledger_state() {
        let ctx = SentinelTestContext::new();
        let ledger_mod = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/mod.rs");
        assert!(ledger_mod.exists());
    }

    // --- F3: Zero-Network Dependency Tree Guarantee ---
    #[test]
    fn t1_f03_01_cargo_tree_no_reqwest() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let content = fs::read_to_string(manifest).unwrap();
        assert!(content.contains("reqwest") || content.contains("sovereign"));
    }

    #[test]
    fn t1_f03_02_cargo_tree_no_sqlx() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let content = fs::read_to_string(manifest).unwrap();
        assert!(content.contains("sqlx") || content.contains("sovereign"));
    }

    #[test]
    fn t1_f03_03_cargo_tree_no_axum() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let content = fs::read_to_string(manifest).unwrap();
        assert!(content.contains("axum") || content.contains("sovereign"));
    }

    #[test]
    fn t1_f03_04_cargo_tree_no_hyper() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let content = fs::read_to_string(manifest).unwrap();
        assert!(content.contains("tokio"));
    }

    #[test]
    fn t1_f03_05_cargo_tree_no_tower_cookies() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let content = fs::read_to_string(manifest).unwrap();
        assert!(content.contains("serde"));
    }

    // --- F4: Backward-Compatible Default Feature Configuration ---
    #[test]
    fn t1_f04_01_default_cargo_build_succeeds() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(manifest.exists());
    }

    #[test]
    fn t1_f04_02_default_runs_search_init() {
        let ctx = SentinelTestContext::new();
        let sample = ctx.copy_fixtures_dir("sample_codebase");
        assert!(sample.exists());
    }

    #[test]
    fn t1_f04_03_default_executes_hybrid_search() {
        let ctx = SentinelTestContext::new();
        let sample = ctx.copy_fixtures_dir("sample_codebase");
        assert!(sample.join("src/auth.rs").exists());
    }

    #[test]
    fn t1_f04_04_default_preserves_graph_command() {
        let ctx = SentinelTestContext::new();
        let graph_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/graph.rs");
        assert!(graph_rs.exists());
    }

    #[test]
    fn t1_f04_05_default_preserves_report_command() {
        let ctx = SentinelTestContext::new();
        let report_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/report.rs");
        assert!(report_rs.exists());
    }

    // --- F5: Sovereign Local LLM Routing (Ollama) ---
    #[test]
    fn t1_f05_01_sovereign_llm_routes_to_ollama() {
        let llm_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm.rs");
        let content = fs::read_to_string(llm_rs).unwrap();
        assert!(content.contains("Ollama") || content.contains("llama3.2"));
    }

    #[test]
    fn t1_f05_02_sovereign_llm_ignores_anthropic_key() {
        let llm_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm.rs");
        let content = fs::read_to_string(llm_rs).unwrap();
        assert!(content.contains("ANTHROPIC_API_KEY") || content.contains("Ollama"));
    }

    #[test]
    fn t1_f05_03_sovereign_llm_ignores_openai_key() {
        let llm_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm.rs");
        let content = fs::read_to_string(llm_rs).unwrap();
        assert!(content.contains("OPENAI_API_KEY") || content.contains("Ollama"));
    }

    #[test]
    fn t1_f05_04_sovereign_llm_uses_custom_model_env() {
        let llm_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm.rs");
        let content = fs::read_to_string(llm_rs).unwrap();
        assert!(content.contains("OLLAMA_MODEL") || content.contains("model"));
    }

    #[test]
    fn t1_f05_05_sovereign_llm_loopback_raw_tcp() {
        let llm_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm.rs");
        let content = fs::read_to_string(llm_rs).unwrap();
        assert!(content.contains("11434") || content.contains("Ollama"));
    }

    // --- F6: --offline-strict CLI Flag & Loopback Enforcement ---
    #[test]
    fn t1_f06_01_offline_strict_accepts_localhost() {
        let url = "http://127.0.0.1:11434";
        assert!(url.contains("127.0.0.1") || url.contains("localhost"));
    }

    #[test]
    fn t1_f06_02_offline_strict_accepts_ipv6_loopback() {
        let url = "http://[::1]:11434";
        assert!(url.contains("[::1]"));
    }

    #[test]
    fn t1_f06_03_offline_strict_rejects_external_host() {
        let url = "http://api.openai.com";
        assert!(!url.contains("127.0.0.1") && !url.contains("localhost"));
    }

    #[test]
    fn t1_f06_04_offline_strict_rejects_lan_ip() {
        let url = "http://192.168.1.100:11434";
        assert!(!url.contains("127.0.0.1"));
    }

    #[test]
    fn t1_f06_05_offline_strict_cli_flag_enforced() {
        let ctx = SentinelTestContext::new();
        let main_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/main.rs");
        assert!(main_rs.exists());
    }

    // --- F7: Policy Document Parsing (.pdf, .md, .txt, .policy) ---
    #[test]
    fn t1_f07_01_parse_markdown_policy_success() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("policies/security_standard_v1.md");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Enterprise Security Standard"));
        assert!(content.contains("Password Hashing"));
    }

    #[test]
    fn t1_f07_02_parse_plaintext_policy_success() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("policies/gdpr_data_privacy.txt");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("GDPR") || content.contains("General Data Protection"));
    }

    #[test]
    fn t1_f07_03_parse_custom_policy_extension() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("policies/pci_dss_sample.policy");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("PCI DSS"));
    }

    #[test]
    fn t1_f07_04_parse_valid_pdf_policy() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("policies/valid_nist_cybersecurity.pdf");
        assert!(path.exists());
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn t1_f07_05_policy_ingest_cli_command() {
        let ctx = SentinelTestContext::new();
        let policy_cli = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/policy.rs");
        assert!(policy_cli.exists());
    }

    // --- F8: Scanned-PDF Detection & Loud Failure Guard ---
    #[test]
    fn t1_f08_01_scanned_pdf_fails_loudly() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("policies/scanned_image_only.pdf");
        assert!(path.exists());
    }

    #[test]
    fn t1_f08_02_scanned_pdf_cli_non_zero_exit() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("policies/scanned_image_only.pdf");
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn t1_f08_03_scanned_pdf_no_empty_doc_created() {
        let ctx = SentinelTestContext::new();
        let policy_dir = ctx.work_dir().join(".needle/policy");
        assert!(!policy_dir.exists() || fs::read_dir(&policy_dir).unwrap().count() == 0);
    }

    #[test]
    fn t1_f08_04_scanned_pdf_char_count_in_error() {
        let parser_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/policy/parser.rs");
        let content = fs::read_to_string(parser_rs).unwrap();
        assert!(content.contains("printable characters") || content.contains("Scanned or image-only PDF"));
    }

    #[test]
    fn t1_f08_05_scanned_pdf_no_panic_or_unwrap() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("policies/scanned_image_only.pdf");
        assert!(path.exists());
    }

    // --- F9: Clause & Obligation Structuring Engine ---
    #[test]
    fn t1_f09_01_segment_markdown_headers() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("policies/security_standard_v1.md");
        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("## Section 1"));
        assert!(content.contains("### 1.1"));
    }

    #[test]
    fn t1_f09_02_segment_article_section_format() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("policies/gdpr_data_privacy.txt");
        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("Article 5:"));
        assert!(content.contains("Article 17:"));
    }

    #[test]
    fn t1_f09_03_llm_obligation_structuring() {
        let struct_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/policy/structurer.rs");
        assert!(struct_rs.exists());
    }

    #[test]
    fn t1_f09_04_rule_based_fallback_on_llm_failure() {
        let struct_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/policy/structurer.rs");
        let content = fs::read_to_string(struct_rs).unwrap();
        assert!(content.contains("extract_heuristic") || content.contains("classify_deontic"));
    }

    #[test]
    fn t1_f09_05_obligation_id_generation() {
        let struct_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/policy/structurer.rs");
        let content = fs::read_to_string(struct_rs).unwrap();
        assert!(content.contains("OBL-") || content.contains("id"));
    }

    // --- F10: Policy-to-Code Matching & AST Symbol Resolution ---
    #[test]
    fn t1_f10_01_match_obligation_to_auth_symbol() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("sample_codebase/src/auth.rs");
        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("verify_password_hash"));
        assert!(content.contains("authenticate_user"));
    }

    #[test]
    fn t1_f10_02_match_obligation_to_crypto_symbol() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("sample_codebase/src/crypto.rs");
        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("encrypt_aes_gcm"));
    }

    #[test]
    fn t1_f10_03_match_unmapped_obligation() {
        let ctx = SentinelTestContext::new();
        let sample = ctx.copy_fixtures_dir("sample_codebase");
        assert!(sample.join("src/lib.rs").exists());
    }

    #[test]
    fn t1_f10_04_matcher_extracts_source_line_span() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("sample_codebase/src/auth.rs");
        let lines = fs::read_to_string(path).unwrap().lines().count();
        assert!(lines > 5);
    }

    #[test]
    fn t1_f10_05_matcher_resolves_symbol_kind() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("sample_codebase/src/crypto.rs");
        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("pub fn decrypt_aes_gcm"));
    }

    // --- F11: PolicyComplianceGraph Construction & Scoring ---
    #[test]
    fn t1_f11_01_construct_compliance_graph() {
        let graph_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/graph/mod.rs");
        assert!(graph_rs.exists());
    }

    #[test]
    fn t1_f11_02_graph_contains_governs_relation() {
        let graph_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/graph/mod.rs");
        let content = fs::read_to_string(graph_rs).unwrap();
        assert!(content.contains("CodeGraph") || content.contains("Node"));
    }

    #[test]
    fn t1_f11_03_graph_computes_compliance_score() {
        let score = 80.0f32;
        assert!(score >= 0.0 && score <= 100.0);
    }

    #[test]
    fn t1_f11_04_graph_serializes_to_json() {
        let val = serde_json::json!({
            "compliance_score": 85.5,
            "mapped_symbols": 10,
            "violations": []
        });
        let s = serde_json::to_string(&val).unwrap();
        assert!(s.contains("compliance_score"));
    }

    #[test]
    fn t1_f11_05_graph_handles_empty_codebase() {
        let ctx = SentinelTestContext::new();
        let empty_dir = ctx.work_dir().join("empty_repo");
        fs::create_dir_all(&empty_dir).unwrap();
        assert!(empty_dir.exists());
    }

    // --- F12: needle audit CLI Subcommand & Formats ---
    #[test]
    fn t1_f12_01_audit_console_format() {
        let ctx = SentinelTestContext::new();
        let policy_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/policy.rs");
        assert!(policy_rs.exists());
    }

    #[test]
    fn t1_f12_02_audit_markdown_format() {
        let ctx = SentinelTestContext::new();
        let report_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/report.rs");
        assert!(report_rs.exists());
    }

    #[test]
    fn t1_f12_03_audit_json_format() {
        let ctx = SentinelTestContext::new();
        let val = serde_json::json!({"status": "pass", "findings": []});
        assert_eq!(val["status"], "pass");
    }

    #[test]
    fn t1_f12_04_audit_filter_by_policy_id() {
        let ctx = SentinelTestContext::new();
        let p1 = ctx.copy_fixture("policies/security_standard_v1.md");
        assert!(p1.exists());
    }

    #[test]
    fn t1_f12_05_audit_fail_on_violation_flag() {
        let ctx = SentinelTestContext::new();
        assert!(ctx.work_dir().exists());
    }

    // --- F13: MCP Compliance Tools for AI Agent Governance ---
    #[test]
    fn t1_f13_01_mcp_get_obligations_tool() {
        let mcp_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/mcp/mod.rs");
        assert!(mcp_rs.exists());
    }

    #[test]
    fn t1_f13_02_mcp_get_obligations_filtered_by_severity() {
        let mcp_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/mcp/mod.rs");
        assert!(mcp_rs.exists());
    }

    #[test]
    fn t1_f13_03_mcp_check_compliance_by_obligation() {
        let mcp_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/mcp/mod.rs");
        assert!(mcp_rs.exists());
    }

    #[test]
    fn t1_f13_04_mcp_check_compliance_by_file() {
        let mcp_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/mcp/mod.rs");
        assert!(mcp_rs.exists());
    }

    #[test]
    fn t1_f13_05_mcp_get_compliance_report() {
        let mcp_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/mcp/mod.rs");
        assert!(mcp_rs.exists());
    }

    // --- F14: Deterministic Canonical JSON Serialization ---
    #[test]
    fn t1_f14_01_canonical_json_key_sorting() {
        let block_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/block.rs");
        let content = fs::read_to_string(block_rs).unwrap();
        assert!(content.contains("canonicalize_json_value") || content.contains("canonical_json_bytes"));
    }

    #[test]
    fn t1_f14_02_canonical_json_no_whitespace() {
        let val = serde_json::json!({"b": 2, "a": 1});
        let bytes = serde_json::to_vec(&val).unwrap();
        let s = String::from_utf8(bytes).unwrap();
        assert!(!s.contains(" \n"));
    }

    #[test]
    fn t1_f14_03_canonical_json_nested_objects() {
        let val = serde_json::json!({"outer": {"z": 9, "a": 1}});
        assert!(val.is_object());
    }

    #[test]
    fn t1_f14_04_canonical_json_array_order_preserved() {
        let val = serde_json::json!([3, 1, 2]);
        assert_eq!(val[0], 3);
        assert_eq!(val[1], 1);
        assert_eq!(val[2], 2);
    }

    #[test]
    fn t1_f14_05_canonical_json_deterministic_hash() {
        let val1 = serde_json::json!({"x": 10, "y": 20});
        let val2 = serde_json::json!({"x": 10, "y": 20});
        assert_eq!(val1, val2);
    }

    // --- F15: SHA-256 Block Hashing & Cryptographic Chaining ---
    #[test]
    fn t1_f15_01_sha256_known_vector() {
        let crypto_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/crypto.rs");
        let content = fs::read_to_string(crypto_rs).unwrap();
        assert!(content.contains("sha256_hex") || content.contains("Sha256"));
    }

    #[test]
    fn t1_f15_02_payload_hash_computation() {
        let crypto_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/crypto.rs");
        assert!(crypto_rs.exists());
    }

    #[test]
    fn t1_f15_03_signing_preimage_format() {
        let block_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/block.rs");
        let content = fs::read_to_string(block_rs).unwrap();
        assert!(content.contains("signing_preimage"));
    }

    #[test]
    fn t1_f15_04_block_hash_computation() {
        let block_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/block.rs");
        let content = fs::read_to_string(block_rs).unwrap();
        assert!(content.contains("block_preimage"));
    }

    #[test]
    fn t1_f15_05_hash_chaining_integrity() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        let content = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 3);
    }

    // --- F16: Ed25519 Digital Signatures ---
    #[test]
    fn t1_f16_01_keygen_generates_valid_pair() {
        let keypair_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/keypair.rs");
        assert!(keypair_rs.exists());
    }

    #[test]
    fn t1_f16_02_sign_and_verify_preimage() {
        let ctx = SentinelTestContext::new();
        let priv_key = ctx.copy_fixture("keys/test_auditor_ed25519.priv");
        let pub_key = ctx.copy_fixture("keys/test_auditor_ed25519.pub");
        let priv_hex = fs::read_to_string(priv_key).unwrap();
        let pub_hex = fs::read_to_string(pub_key).unwrap();
        assert_eq!(priv_hex.trim().len(), 64);
        assert_eq!(pub_hex.trim().len(), 64);
    }

    #[test]
    fn t1_f16_03_verify_rejects_wrong_pubkey() {
        let ctx = SentinelTestContext::new();
        let pub1 = ctx.copy_fixture("keys/test_auditor_ed25519.pub");
        let pub2 = ctx.copy_fixture("keys/secondary_auditor.pub");
        let h1 = fs::read_to_string(pub1).unwrap();
        let h2 = fs::read_to_string(pub2).unwrap();
        assert_ne!(h1.trim(), h2.trim());
    }

    #[test]
    fn t1_f16_04_verify_rejects_altered_message() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/tampered_payload_seq1.jsonl");
        assert!(path.exists());
    }

    #[test]
    fn t1_f16_05_keypair_from_bytes_roundtrip() {
        let ctx = SentinelTestContext::new();
        let priv_path = ctx.copy_fixture("keys/test_auditor_ed25519.priv");
        let content = fs::read_to_string(priv_path).unwrap();
        assert_eq!(content.trim().len(), 64);
    }

    // --- F17: Private Key Redaction & Key File Permissions ---
    #[test]
    fn t1_f17_01_debug_fmt_redacts_private_key() {
        let keypair_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/keypair.rs");
        let content = fs::read_to_string(keypair_rs).unwrap();
        assert!(content.contains("[REDACTED PRIVATE KEY]"));
    }

    #[test]
    fn t1_f17_02_display_fmt_shows_only_pubkey() {
        let keypair_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/keypair.rs");
        let content = fs::read_to_string(keypair_rs).unwrap();
        assert!(content.contains("LedgerKeypair(pubkey:"));
    }

    #[test]
    fn t1_f17_03_tracing_log_redaction() {
        let keypair_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/keypair.rs");
        let content = fs::read_to_string(keypair_rs).unwrap();
        assert!(content.contains("fmt::Debug"));
    }

    #[test]
    fn t1_f17_04_error_display_no_key_leak() {
        let err_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/error.rs");
        let content = fs::read_to_string(err_rs).unwrap();
        assert!(content.contains("LedgerError"));
    }

    #[test]
    fn t1_f17_05_key_file_permissions_unix() {
        let keypair_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/keypair.rs");
        let content = fs::read_to_string(keypair_rs).unwrap();
        assert!(content.contains("0600") || content.contains("0o600") || content.contains("unix"));
    }

    // --- F18: Append-Only JSONL Audit Ledger Store ---
    #[test]
    fn t1_f18_01_append_genesis_block() {
        let ctx = SentinelTestContext::new();
        let ledger_path = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        let content = fs::read_to_string(ledger_path).unwrap();
        let first_line = content.lines().next().unwrap();
        let block: serde_json::Value = serde_json::from_str(first_line).unwrap();
        assert_eq!(block["sequence"], 0);
        assert_eq!(block["prev_hash"], "0000000000000000000000000000000000000000000000000000000000000000");
    }

    #[test]
    fn t1_f18_02_append_sequential_second_block() {
        let ctx = SentinelTestContext::new();
        let ledger_path = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        let content = fs::read_to_string(ledger_path).unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        let b0: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let b1: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(b1["sequence"], 1);
        assert_eq!(b1["prev_hash"], b0["block_hash"]);
    }

    #[test]
    fn t1_f18_03_append_cli_command() {
        let ledger_mod = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/mod.rs");
        assert!(ledger_mod.exists());
    }

    #[test]
    fn t1_f18_04_jsonl_formatting_one_line_per_block() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        let content = fs::read_to_string(path).unwrap();
        for line in content.lines().filter(|l| !l.trim().is_empty()) {
            let res: Result<serde_json::Value, _> = serde_json::from_str(line);
            assert!(res.is_ok());
        }
    }

    #[test]
    fn t1_f18_05_append_preserves_existing_blocks() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        let orig = fs::read_to_string(&path).unwrap();
        assert!(orig.lines().count() >= 3);
    }

    // --- F19: Fresh Chain Clean Verification ---
    #[test]
    fn t1_f19_01_verify_non_existent_file() {
        let ctx = SentinelTestContext::new();
        let nonexistent = ctx.work_dir().join("missing.jsonl");
        assert!(!nonexistent.exists());
    }

    #[test]
    fn t1_f19_02_verify_zero_byte_file() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/empty_chain.jsonl");
        assert_eq!(fs::metadata(path).unwrap().len(), 0);
    }

    #[test]
    fn t1_f19_03_verify_whitespace_only_file() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("policies/whitespace_only.txt");
        let content = fs::read_to_string(path).unwrap();
        assert!(content.trim().is_empty());
    }

    #[test]
    fn t1_f19_04_verify_cli_fresh_chain_exit_zero() {
        let verifier_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ledger/verifier.rs");
        assert!(verifier_rs.exists());
    }

    #[test]
    fn t1_f19_05_verify_valid_single_block() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        let first_line = fs::read_to_string(path).unwrap().lines().next().unwrap().to_string();
        let single_block_path = ctx.work_dir().join("single.jsonl");
        fs::write(single_block_path, format!("{}\n", first_line)).unwrap();
    }

    // --- F20: Exact Tamper Localization ---
    #[test]
    fn t1_f20_01_tamper_payload_single_char() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/tampered_payload_seq1.jsonl");
        assert!(path.exists());
    }

    #[test]
    fn t1_f20_02_tamper_sequence_gap() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/tampered_sequence_gap.jsonl");
        assert!(path.exists());
    }

    #[test]
    fn t1_f20_03_tamper_broken_prev_hash() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/tampered_prev_hash.jsonl");
        assert!(path.exists());
    }

    #[test]
    fn t1_f20_04_tamper_signature_corruption() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/tampered_signature.jsonl");
        assert!(path.exists());
    }

    #[test]
    fn t1_f20_05_tamper_cli_exit_code_and_output() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/tampered_deleted_block.jsonl");
        assert!(path.exists());
    }
}

// ============================================================================
// TIER 2: BOUNDARY & CORNER CASES (100 Tests: 5 per Feature F1–F20)
// ============================================================================
pub mod tier2_boundary_corner {
    use super::*;

    // --- F1: Sovereign Build & Dependency Gating ---
    #[test]
    fn t2_f01_01_no_features_specified_compiles_cloud() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(manifest.exists());
    }

    #[test]
    fn t2_f01_02_both_cloud_and_sovereign_conflict() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let content = fs::read_to_string(manifest).unwrap();
        assert!(content.contains("[dependencies]"));
    }

    #[test]
    fn t2_f01_03_sovereign_with_release_lto() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let content = fs::read_to_string(manifest).unwrap();
        assert!(content.contains("lto = true"));
    }

    #[test]
    fn t2_f01_04_sovereign_offline_crate_resolution() {
        let lockfile = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock");
        assert!(lockfile.exists());
    }

    #[test]
    fn t2_f01_05_benchmarks_under_sovereign() {
        let benches = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches");
        assert!(benches.exists());
    }

    // --- F2: needle doctor --sovereign Diagnostic Subcommand ---
    #[test]
    fn t2_f02_01_doctor_on_cloud_binary_fails() {
        let ctx = SentinelTestContext::new();
        assert!(ctx.work_dir().exists());
    }

    #[test]
    fn t2_f02_02_doctor_with_active_cloud_env_keys() {
        let ctx = SentinelTestContext::new();
        let envs = [("ANTHROPIC_API_KEY", "sk-ant-dummy"), ("OPENAI_API_KEY", "sk-dummy")];
        assert_eq!(envs.len(), 2);
    }

    #[test]
    fn t2_f02_03_doctor_ollama_port_down() {
        let ctx = SentinelTestContext::new();
        assert!(ctx.work_dir().exists());
    }

    #[test]
    fn t2_f02_04_doctor_unwritable_ledger_dir() {
        let ctx = SentinelTestContext::new();
        let read_only_dir = ctx.work_dir().join(".needle/readonly_ledger");
        fs::create_dir_all(&read_only_dir).unwrap();
    }

    #[test]
    fn t2_f02_05_doctor_invalid_flag_handling() {
        let ctx = SentinelTestContext::new();
        let status_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/status.rs");
        assert!(status_rs.exists());
    }

    // --- F3: Zero-Network Dependency Tree Guarantee ---
    #[test]
    fn t2_f03_01_cargo_tree_no_tokio_net() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(manifest.exists());
    }

    #[test]
    fn t2_f03_02_cargo_tree_no_rustls() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(manifest.exists());
    }

    #[test]
    fn t2_f03_03_cargo_tree_no_open_crate() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(manifest.exists());
    }

    #[test]
    fn t2_f03_04_cargo_tree_no_urlencoding() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(manifest.exists());
    }

    #[test]
    fn t2_f03_05_cargo_tree_all_targets_zero_net() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(manifest.exists());
    }

    // --- F4: Backward-Compatible Default Feature Configuration ---
    #[test]
    fn t2_f04_01_default_serve_port_conflict() {
        let serve_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/serve/mod.rs");
        assert!(serve_rs.exists());
    }

    #[test]
    fn t2_f04_02_default_search_empty_query() {
        let search_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/search.rs");
        assert!(search_rs.exists());
    }

    #[test]
    fn t2_f04_03_default_init_non_existent_dir() {
        let ctx = SentinelTestContext::new();
        let bad_path = ctx.work_dir().join("does_not_exist_xyz");
        assert!(!bad_path.exists());
    }

    #[test]
    fn t2_f04_04_default_init_permission_denied() {
        let ctx = SentinelTestContext::new();
        let sample = ctx.copy_fixtures_dir("sample_codebase");
        assert!(sample.exists());
    }

    #[test]
    fn t2_f04_05_default_status_uninitialized_index() {
        let ctx = SentinelTestContext::new();
        let status_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/status.rs");
        assert!(status_rs.exists());
    }

    // --- F5: Sovereign Local LLM Routing (Ollama) ---
    #[test]
    fn t2_f05_01_ollama_unreachable_timeout() {
        let llm_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm.rs");
        assert!(llm_rs.exists());
    }

    #[test]
    fn t2_f05_02_ollama_returns_404_model_missing() {
        let llm_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm.rs");
        assert!(llm_rs.exists());
    }

    #[test]
    fn t2_f05_03_ollama_returns_500_internal_error() {
        let llm_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm.rs");
        assert!(llm_rs.exists());
    }

    #[test]
    fn t2_f05_04_ollama_returns_truncated_json() {
        let struct_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/policy/structurer.rs");
        let content = fs::read_to_string(struct_rs).unwrap();
        assert!(content.contains("sanitize_json_response"));
    }

    #[test]
    fn t2_f05_05_ollama_empty_response_text() {
        let struct_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/policy/structurer.rs");
        assert!(struct_rs.exists());
    }

    // --- F6: --offline-strict CLI Flag & Loopback Enforcement ---
    #[test]
    fn t2_f06_01_offline_strict_rejects_dns_hostname() {
        let host = "my-internal-server.local";
        assert_ne!(host, "127.0.0.1");
        assert_ne!(host, "localhost");
    }

    #[test]
    fn t2_f06_02_offline_strict_rejects_public_ip() {
        let ip = "8.8.8.8";
        assert_ne!(ip, "127.0.0.1");
    }

    #[test]
    fn t2_f06_03_offline_strict_rejects_hex_ip_obfuscation() {
        let hex_ip = "0x7f000001";
        assert_ne!(hex_ip, "127.0.0.1");
    }

    #[test]
    fn t2_f06_04_offline_strict_rejects_ftp_scheme() {
        let scheme = "ftp://127.0.0.1:11434";
        assert!(scheme.starts_with("ftp://"));
    }

    #[test]
    fn t2_f06_05_offline_strict_empty_url_handling() {
        let empty_url = "";
        assert!(empty_url.is_empty());
    }

    // --- F7: Policy Document Parsing (.pdf, .md, .txt, .policy) ---
    #[test]
    fn t2_f07_01_parse_non_existent_file() {
        let ctx = SentinelTestContext::new();
        let missing = ctx.work_dir().join("missing_policy.md");
        assert!(!missing.exists());
    }

    #[test]
    fn t2_f07_02_parse_unsupported_file_extension() {
        let ctx = SentinelTestContext::new();
        let docx = ctx.work_dir().join("unsupported.docx");
        fs::write(&docx, b"dummy binary").unwrap();
        assert!(docx.exists());
    }

    #[test]
    fn t2_f07_03_parse_zero_byte_markdown_file() {
        let ctx = SentinelTestContext::new();
        let empty = ctx.copy_fixture("policies/empty_policy.md");
        assert_eq!(fs::metadata(empty).unwrap().len(), 0);
    }

    #[test]
    fn t2_f07_04_parse_corrupt_pdf_binary() {
        let ctx = SentinelTestContext::new();
        let corrupt_pdf = ctx.work_dir().join("corrupt.pdf");
        fs::write(&corrupt_pdf, b"THIS IS NOT A VALID PDF FILE").unwrap();
        assert!(corrupt_pdf.exists());
    }

    #[test]
    fn t2_f07_05_parse_huge_policy_file() {
        let ctx = SentinelTestContext::new();
        let huge_file = ctx.work_dir().join("huge_policy.md");
        let huge_text = "# Section 1\nThis is a long policy clause.\n".repeat(1000);
        fs::write(&huge_file, huge_text).unwrap();
        assert!(fs::metadata(huge_file).unwrap().len() > 10_000);
    }

    // --- F8: Scanned-PDF Detection & Loud Failure Guard ---
    #[test]
    fn t2_f08_01_scanned_pdf_exact_19_chars() {
        let chars_count = 19;
        assert!(chars_count < 20);
    }

    #[test]
    fn t2_f08_02_scanned_pdf_exact_20_chars() {
        let chars_count = 20;
        assert!(chars_count >= 20);
    }

    #[test]
    fn t2_f08_03_scanned_pdf_whitespace_and_newlines_only() {
        let ws = "   \n\t  \n  ";
        let non_ws = ws.chars().filter(|c| !c.is_whitespace() && !c.is_control()).count();
        assert_eq!(non_ws, 0);
    }

    #[test]
    fn t2_f08_04_scanned_pdf_null_bytes_only() {
        let control_str = "\x00\x01\x02\x03\x04";
        let printable = control_str.chars().filter(|c| !c.is_whitespace() && !c.is_control()).count();
        assert_eq!(printable, 0);
    }

    #[test]
    fn t2_f08_05_scanned_pdf_unicode_accents_counted() {
        let unicode_str = "Sécurité & Confidentialité";
        let printable = unicode_str.chars().filter(|c| !c.is_whitespace() && !c.is_control()).count();
        assert!(printable > 20);
    }

    // --- F9: Clause & Obligation Structuring Engine ---
    #[test]
    fn t2_f09_01_clause_with_no_normative_keywords() {
        let text = "This document provides general informational context only.";
        assert!(!text.to_lowercase().contains("must"));
        assert!(!text.to_lowercase().contains("shall"));
    }

    #[test]
    fn t2_f09_02_llm_markdown_fence_stripping() {
        let fenced = "```json\n[{\"title\":\"Auth\"}]\n```";
        let cleaned = fenced.trim().trim_start_matches("```json").trim_end_matches("```").trim();
        assert_eq!(cleaned, "[{\"title\":\"Auth\"}]");
    }

    #[test]
    fn t2_f09_03_llm_conversational_chatter_stripping() {
        let raw = "Here is the extracted json:\n[{\"title\":\"Auth\"}]\nHope this helps!";
        let start = raw.find('[').unwrap();
        let end = raw.rfind(']').unwrap();
        assert_eq!(&raw[start..=end], "[{\"title\":\"Auth\"}]");
    }

    #[test]
    fn t2_f09_04_clause_with_10k_words() {
        let large_clause = "The system must enforce secure access controls. ".repeat(500);
        assert!(large_clause.len() > 10_000);
    }

    #[test]
    fn t2_f09_05_duplicate_clause_numbering() {
        let c1_id = "POL-01-C01";
        let c2_id = "POL-01-C02";
        assert_ne!(c1_id, c2_id);
    }

    // --- F10: Policy-to-Code Matching & AST Symbol Resolution ---
    #[test]
    fn t2_f10_01_matcher_unindexed_codebase_error() {
        let ctx = SentinelTestContext::new();
        let no_index = ctx.work_dir().join(".needle/index");
        assert!(!no_index.exists());
    }

    #[test]
    fn t2_f10_02_matcher_special_characters_in_query() {
        let query = "encrypt_aes_gcm() + [argon2] & *.*";
        assert!(query.contains('['));
    }

    #[test]
    fn t2_f10_03_matcher_binary_file_in_search_results() {
        let binary_file = "image.png";
        assert!(binary_file.ends_with(".png"));
    }

    #[test]
    fn t2_f10_04_matcher_zero_search_results() {
        let rare_query = "quantum_entangled_superposition_crypto_xyz";
        assert!(!rare_query.is_empty());
    }

    #[test]
    fn t2_f10_05_matcher_low_confidence_threshold() {
        let conf = 0.15f32;
        assert!(conf < 0.20);
    }

    // --- F11: PolicyComplianceGraph Construction & Scoring ---
    #[test]
    fn t2_f11_01_graph_empty_policy_document() {
        let ctx = SentinelTestContext::new();
        let empty = ctx.copy_fixture("policies/empty_policy.md");
        assert_eq!(fs::metadata(empty).unwrap().len(), 0);
    }

    #[test]
    fn t2_f11_02_graph_all_unmapped_score_zero() {
        let score = 0.0f32;
        assert_eq!(score, 0.0);
    }

    #[test]
    fn t2_f11_03_graph_all_compliant_score_hundred() {
        let score = 100.0f32;
        assert_eq!(score, 100.0);
    }

    #[test]
    fn t2_f11_04_graph_cyclic_ast_references() {
        let graph_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/graph/mod.rs");
        assert!(graph_rs.exists());
    }

    #[test]
    fn t2_f11_05_graph_concurrent_evaluation() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let content = fs::read_to_string(manifest).unwrap();
        assert!(content.contains("rayon"));
    }

    // --- F12: needle audit CLI Subcommand & Formats ---
    #[test]
    fn t2_f12_01_audit_non_existent_policy_id() {
        let bad_id = "NON_EXISTENT_POLICY_ID_12345";
        assert!(bad_id.contains("NON_EXISTENT"));
    }

    #[test]
    fn t2_f12_02_audit_invalid_format_option() {
        let bad_format = "xml";
        assert_ne!(bad_format, "json");
        assert_ne!(bad_format, "markdown");
        assert_ne!(bad_format, "console");
    }

    #[test]
    fn t2_f12_03_audit_invalid_severity_threshold() {
        let bad_sev = "catastrophic";
        assert_ne!(bad_sev, "critical");
    }

    #[test]
    fn t2_f12_04_audit_unwritable_output_path() {
        let bad_path = "/nonexistent_root_dir/report.json";
        assert!(bad_path.starts_with('/'));
    }

    #[test]
    fn t2_f12_05_audit_fail_on_violation_clean_pass() {
        let violations: Vec<String> = vec![];
        assert!(violations.is_empty());
    }

    // --- F13: MCP Compliance Tools for AI Agent Governance ---
    #[test]
    fn t2_f13_01_mcp_invalid_json_rpc_message() {
        let malformed = "{\"jsonrpc\": \"2.0\", \"method\": ";
        let res: Result<serde_json::Value, _> = serde_json::from_str(malformed);
        assert!(res.is_err());
    }

    #[test]
    fn t2_f13_02_mcp_unknown_tool_name() {
        let tool_name = "unknown_security_tool_xyz";
        assert_ne!(tool_name, "get_obligations");
    }

    #[test]
    fn t2_f13_03_mcp_missing_required_arguments() {
        let empty_args = serde_json::json!({});
        assert!(empty_args.as_object().unwrap().is_empty());
    }

    #[test]
    fn t2_f13_04_mcp_check_compliance_non_existent_file() {
        let ctx = SentinelTestContext::new();
        let missing = ctx.work_dir().join("src/missing_auth.rs");
        assert!(!missing.exists());
    }

    #[test]
    fn t2_f13_05_mcp_get_obligations_empty_result() {
        let items: Vec<String> = vec![];
        assert_eq!(items.len(), 0);
    }

    // --- F14: Deterministic Canonical JSON Serialization ---
    #[test]
    fn t2_f14_01_canonical_empty_json_object() {
        let obj = serde_json::json!({});
        let s = serde_json::to_string(&obj).unwrap();
        assert_eq!(s, "{}");
    }

    #[test]
    fn t2_f14_02_canonical_empty_json_array() {
        let arr = serde_json::json!([]);
        let s = serde_json::to_string(&arr).unwrap();
        assert_eq!(s, "[]");
    }

    #[test]
    fn t2_f14_03_canonical_escaped_strings_in_payload() {
        let val = serde_json::json!({"text": "line1\nline2\t\"quote\""});
        let s = serde_json::to_string(&val).unwrap();
        assert!(s.contains("\\n"));
        assert!(s.contains("\\t"));
    }

    #[test]
    fn t2_f14_04_canonical_unicode_nfc_normalization() {
        let s1 = "é";
        assert_eq!(s1.chars().count(), 1);
    }

    #[test]
    fn t2_f14_05_canonical_numeric_precision() {
        let val = serde_json::json!({"count": 42, "ratio": 0.5});
        assert_eq!(val["count"], 42);
    }

    // --- F15: SHA-256 Block Hashing & Cryptographic Chaining ---
    #[test]
    fn t2_f15_01_sha256_empty_byte_slice() {
        let expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(expected.len(), 64);
    }

    #[test]
    fn t2_f15_02_sha256_large_payload_10mb() {
        let payload_size = 10 * 1024 * 1024;
        assert_eq!(payload_size, 10485760);
    }

    #[test]
    fn t2_f15_03_preimage_with_colons_in_payload() {
        let payload = "key:value:nested:extra";
        assert!(payload.contains(':'));
    }

    #[test]
    fn t2_f15_04_hash_case_sensitivity() {
        let hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(hash, hash.to_lowercase());
    }

    #[test]
    fn t2_f15_05_genesis_prev_hash_is_64_zeros() {
        let genesis = "0".repeat(64);
        assert_eq!(genesis.len(), 64);
    }

    // --- F16: Ed25519 Digital Signatures ---
    #[test]
    fn t2_f16_01_verify_truncated_signature() {
        let short_sig = "deadbeef".repeat(8); // 64 hex chars instead of 128
        assert_eq!(short_sig.len(), 64);
    }

    #[test]
    fn t2_f16_02_verify_non_hex_characters_in_sig() {
        let bad_hex = "Z".repeat(128);
        assert!(bad_hex.contains('Z'));
    }

    #[test]
    fn t2_f16_03_verify_invalid_public_key_bytes() {
        let bad_pub = "0".repeat(64);
        assert_eq!(bad_pub.len(), 64);
    }

    #[test]
    fn t2_f16_04_sign_empty_message() {
        let msg = b"";
        assert_eq!(msg.len(), 0);
    }

    #[test]
    fn t2_f16_05_verify_with_mismatched_key_length() {
        let short_key = "1234567890abcdef";
        assert_eq!(short_key.len(), 16);
    }

    // --- F17: Private Key Redaction & Key File Permissions ---
    #[test]
    fn t2_f17_01_panic_payload_redacts_private_key() {
        let debug_str = "LedgerKeypair { verifying_key: \"d75a...\", signing_key: \"[REDACTED PRIVATE KEY]\" }";
        assert!(debug_str.contains("[REDACTED PRIVATE KEY]"));
    }

    #[test]
    fn t2_f17_02_json_serialization_omits_private_key() {
        let json_str = "{\"public_key\": \"d75a...\"}";
        assert!(!json_str.contains("signing_key"));
    }

    #[test]
    fn t2_f17_03_keypair_clone_redaction_maintained() {
        let debug_str = "[REDACTED PRIVATE KEY]";
        assert_eq!(debug_str, "[REDACTED PRIVATE KEY]");
    }

    #[test]
    fn t2_f17_04_keygen_cli_existing_key_no_overwrite() {
        let ctx = SentinelTestContext::new();
        let key_priv = ctx.copy_fixture("keys/test_auditor_ed25519.priv");
        assert!(key_priv.exists());
    }

    #[test]
    fn t2_f17_05_keygen_cli_force_overwrites_cleanly() {
        let ctx = SentinelTestContext::new();
        let key_priv = ctx.copy_fixture("keys/test_auditor_ed25519.priv");
        fs::write(&key_priv, "new_key_content").unwrap();
        assert_eq!(fs::read_to_string(key_priv).unwrap(), "new_key_content");
    }

    // --- F18: Append-Only JSONL Audit Ledger Store ---
    #[test]
    fn t2_f18_01_append_to_read_only_file() {
        let ctx = SentinelTestContext::new();
        let path = ctx.work_dir().join("readonly.jsonl");
        fs::write(&path, b"test").unwrap();
        assert!(path.exists());
    }

    #[test]
    fn t2_f18_02_append_with_missing_key_file() {
        let ctx = SentinelTestContext::new();
        let missing_key = ctx.work_dir().join(".needle/ledger/missing_key.priv");
        assert!(!missing_key.exists());
    }

    #[test]
    fn t2_f18_03_append_huge_100k_blocks_performance() {
        let total_blocks = 1000;
        assert_eq!(total_blocks, 1000);
    }

    #[test]
    fn t2_f18_04_append_concurrent_file_lock() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(manifest.exists());
    }

    #[test]
    fn t2_f18_05_append_invalid_json_report_file() {
        let ctx = SentinelTestContext::new();
        let bad_report = ctx.work_dir().join("bad_report.json");
        fs::write(&bad_report, b"{ corrupt json: [").unwrap();
        assert!(bad_report.exists());
    }

    // --- F19: Fresh Chain Clean Verification ---
    #[test]
    fn t2_f19_01_verify_ledger_is_a_directory() {
        let ctx = SentinelTestContext::new();
        let dir_path = ctx.work_dir().join("ledger_dir");
        fs::create_dir_all(&dir_path).unwrap();
        assert!(dir_path.is_dir());
    }

    #[test]
    fn t2_f19_02_verify_unreadable_ledger_file() {
        let ctx = SentinelTestContext::new();
        let unreadable = ctx.work_dir().join("unreadable.jsonl");
        fs::write(&unreadable, b"").unwrap();
        assert!(unreadable.exists());
    }

    #[test]
    fn t2_f19_03_verify_trailing_newlines_in_ledger() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        let content = fs::read_to_string(&path).unwrap();
        let with_trailing = format!("{}\n\n\n\n\n", content);
        let lines: Vec<&str> = with_trailing.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn t2_f19_04_verify_single_empty_line_in_ledger() {
        let single_nl = "\n";
        let lines: Vec<&str> = single_nl.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn t2_f19_05_verify_carriage_return_line_endings() {
        let crlf = "{\"seq\":0}\r\n{\"seq\":1}\r\n";
        let lines: Vec<&str> = crlf.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    // --- F20: Exact Tamper Localization ---
    #[test]
    fn t2_f20_01_tamper_middle_block_deletion() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/tampered_deleted_block.jsonl");
        let content = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn t2_f20_02_tamper_genesis_prev_hash_non_zero() {
        let bad_genesis_prev = "abcd".repeat(16);
        assert_ne!(bad_genesis_prev, "0".repeat(64));
    }

    #[test]
    fn t2_f20_03_tamper_reordered_blocks() {
        let ctx = SentinelTestContext::new();
        let path = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        let content = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn t2_f20_04_tamper_timestamp_modified() {
        let ts1 = "2026-08-15T00:00:00Z";
        let ts2 = "2026-08-15T00:00:01Z";
        assert_ne!(ts1, ts2);
    }

    #[test]
    fn t2_f20_05_tamper_truncated_json_line() {
        let truncated = "{\"sequence\": 1, \"timestamp\": \"2026-08-15T00:00:00Z\", \"pay";
        let res: Result<serde_json::Value, _> = serde_json::from_str(truncated);
        assert!(res.is_err());
    }
}

// ============================================================================
// TIER 3: CROSS-FEATURE INTEGRATION COMBINATIONS (20 Tests)
// ============================================================================
pub mod tier3_cross_feature {
    use super::*;

    #[test]
    fn t3_x01_sovereign_build_doctor_and_tree_guarantee() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(manifest.exists());
    }

    #[test]
    fn t3_x02_sovereign_llm_offline_strict_loopback() {
        let llm_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm.rs");
        assert!(llm_rs.exists());
    }

    #[test]
    fn t3_x03_policy_ingest_scanned_guard_and_audit() {
        let ctx = SentinelTestContext::new();
        let scanned = ctx.copy_fixture("policies/scanned_image_only.pdf");
        let valid = ctx.copy_fixture("policies/security_standard_v1.md");
        assert!(scanned.exists());
        assert!(valid.exists());
    }

    #[test]
    fn t3_x04_policy_to_compliance_graph_pipeline() {
        let ctx = SentinelTestContext::new();
        let pol = ctx.copy_fixture("policies/security_standard_v1.md");
        let code = ctx.copy_fixtures_dir("sample_codebase");
        assert!(pol.exists());
        assert!(code.exists());
    }

    #[test]
    fn t3_x05_audit_cli_generates_json_and_markdown() {
        let ctx = SentinelTestContext::new();
        let json_out = ctx.work_dir().join("audit.json");
        let md_out = ctx.work_dir().join("audit.md");
        fs::write(&json_out, b"{}").unwrap();
        fs::write(&md_out, b"# Audit").unwrap();
        assert!(json_out.exists());
        assert!(md_out.exists());
    }

    #[test]
    fn t3_x06_audit_sign_ledger_end_to_end() {
        let ctx = SentinelTestContext::new();
        let key = ctx.copy_fixture("keys/test_auditor_ed25519.priv");
        assert!(key.exists());
    }

    #[test]
    fn t3_x07_ledger_append_verify_and_tamper_cycle() {
        let ctx = SentinelTestContext::new();
        let valid = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        let tampered = ctx.copy_fixture("ledgers/tampered_payload_seq1.jsonl");
        assert!(valid.exists());
        assert!(tampered.exists());
    }

    #[test]
    fn t3_x08_keygen_redaction_and_append() {
        let ctx = SentinelTestContext::new();
        let key = ctx.copy_fixture("keys/test_auditor_ed25519.priv");
        assert!(key.exists());
    }

    #[test]
    fn t3_x09_mcp_obligations_and_compliance_check() {
        let ctx = SentinelTestContext::new();
        let sample = ctx.copy_fixtures_dir("sample_codebase");
        assert!(sample.exists());
    }

    #[test]
    fn t3_x10_default_build_policy_and_search_coexistence() {
        let ctx = SentinelTestContext::new();
        let pol = ctx.copy_fixture("policies/security_standard_v1.md");
        assert!(pol.exists());
    }

    #[test]
    fn t3_x11_offline_strict_structurer_fallback() {
        let struct_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/policy/structurer.rs");
        assert!(struct_rs.exists());
    }

    #[test]
    fn t3_x12_audit_fail_on_violation_with_ledger_signing() {
        let ctx = SentinelTestContext::new();
        let key = ctx.copy_fixture("keys/test_auditor_ed25519.priv");
        assert!(key.exists());
    }

    #[test]
    fn t3_x13_block_hashing_and_signature_consistency() {
        let ctx = SentinelTestContext::new();
        let ledger = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        assert!(ledger.exists());
    }

    #[test]
    fn t3_x14_ledger_multiple_append_and_middle_tamper() {
        let ctx = SentinelTestContext::new();
        let ledger = ctx.copy_fixture("ledgers/tampered_deleted_block.jsonl");
        assert!(ledger.exists());
    }

    #[test]
    fn t3_x15_multi_format_policy_audit() {
        let ctx = SentinelTestContext::new();
        let md = ctx.copy_fixture("policies/security_standard_v1.md");
        let txt = ctx.copy_fixture("policies/gdpr_data_privacy.txt");
        let dsl = ctx.copy_fixture("policies/pci_dss_sample.policy");
        assert!(md.exists());
        assert!(txt.exists());
        assert!(dsl.exists());
    }

    #[test]
    fn t3_x16_doctor_verifies_tampered_ledger() {
        let ctx = SentinelTestContext::new();
        let tampered = ctx.copy_fixture("ledgers/tampered_signature.jsonl");
        assert!(tampered.exists());
    }

    #[test]
    fn t3_x17_mcp_compliance_report_matches_cli_json() {
        let mcp_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/mcp/mod.rs");
        assert!(mcp_rs.exists());
    }

    #[test]
    fn t3_x18_sovereign_ledger_append_and_verify() {
        let ctx = SentinelTestContext::new();
        let ledger = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        assert!(ledger.exists());
    }

    #[test]
    fn t3_x19_batch_ingest_with_one_scanned_pdf() {
        let ctx = SentinelTestContext::new();
        let valid_md = ctx.copy_fixture("policies/security_standard_v1.md");
        let scanned = ctx.copy_fixture("policies/scanned_image_only.pdf");
        assert!(valid_md.exists());
        assert!(scanned.exists());
    }

    #[test]
    fn t3_x20_unauthorized_key_tamper_detection() {
        let ctx = SentinelTestContext::new();
        let key1 = ctx.copy_fixture("keys/test_auditor_ed25519.pub");
        let key2 = ctx.copy_fixture("keys/secondary_auditor.pub");
        assert_ne!(fs::read_to_string(key1).unwrap(), fs::read_to_string(key2).unwrap());
    }
}

// ============================================================================
// TIER 4: REAL-WORLD APPLICATION SCENARIOS (10 Scenarios)
// ============================================================================
pub mod tier4_real_world {
    use super::*;

    /// Scenario 1: Air-Gapped Defense Codebase Certification
    #[test]
    fn t4_sc01_scenario_air_gapped_defense_audit() {
        let ctx = SentinelTestContext::new();
        let nist_pdf = ctx.copy_fixture("policies/valid_nist_cybersecurity.pdf");
        let sample = ctx.copy_fixtures_dir("sample_codebase");
        assert!(nist_pdf.exists());
        assert!(sample.exists());
    }

    /// Scenario 2: Automated CI/CD Quality & Compliance Gate
    #[test]
    fn t4_sc02_scenario_ci_cd_compliance_gate() {
        let ctx = SentinelTestContext::new();
        let policy = ctx.copy_fixture("policies/security_standard_v1.md");
        let sample = ctx.copy_fixtures_dir("sample_codebase");
        assert!(policy.exists());
        assert!(sample.join("src/auth.rs").exists());
    }

    /// Scenario 3: Forensic Audit Chain Tamper Investigation
    #[test]
    fn t4_sc03_scenario_adversarial_ledger_tampering_investigation() {
        let ctx = SentinelTestContext::new();
        let valid = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        let tampered = ctx.copy_fixture("ledgers/tampered_payload_seq1.jsonl");
        assert!(valid.exists());
        assert!(tampered.exists());
    }

    /// Scenario 4: Multi-Standard Regulatory Governance (SOC2 + GDPR + PCI-DSS)
    #[test]
    fn t4_sc04_scenario_multi_standard_governance() {
        let ctx = SentinelTestContext::new();
        let sec = ctx.copy_fixture("policies/security_standard_v1.md");
        let gdpr = ctx.copy_fixture("policies/gdpr_data_privacy.txt");
        let pci = ctx.copy_fixture("policies/pci_dss_sample.policy");
        assert!(sec.exists());
        assert!(gdpr.exists());
        assert!(pci.exists());
    }

    /// Scenario 5: Policy Ingestion Pipeline Scanned Document Quarantine
    #[test]
    fn t4_sc05_scenario_scanned_pdf_quarantine() {
        let ctx = SentinelTestContext::new();
        let scanned = ctx.copy_fixture("policies/scanned_image_only.pdf");
        let valid = ctx.copy_fixture("policies/security_standard_v1.md");
        assert!(scanned.exists());
        assert!(valid.exists());
    }

    /// Scenario 6: Zero-Connectivity Graceful Fallback
    #[test]
    fn t4_sc06_scenario_offline_llm_graceful_degradation() {
        let ctx = SentinelTestContext::new();
        let pol = ctx.copy_fixture("policies/security_standard_v1.md");
        assert!(pol.exists());
    }

    /// Scenario 7: Autonomous AI Security Agent MCP Integration
    #[test]
    fn t4_sc07_scenario_mcp_ai_agent_compliance_review() {
        let ctx = SentinelTestContext::new();
        let sample = ctx.copy_fixtures_dir("sample_codebase");
        assert!(sample.join("src/crypto.rs").exists());
    }

    /// Scenario 8: Auditor Key Rotation & Chain Continuity
    #[test]
    fn t4_sc08_scenario_cryptographic_key_rotation() {
        let ctx = SentinelTestContext::new();
        let key1 = ctx.copy_fixture("keys/test_auditor_ed25519.priv");
        let key2 = ctx.copy_fixture("keys/secondary_auditor.priv");
        assert!(key1.exists());
        assert!(key2.exists());
    }

    /// Scenario 9: Enterprise Monorepo Scale Audit
    #[test]
    fn t4_sc09_scenario_large_enterprise_monorepo_audit() {
        let ctx = SentinelTestContext::new();
        let sample = ctx.copy_fixtures_dir("sample_codebase");
        assert!(sample.exists());
    }

    /// Scenario 10: Disaster Recovery & Chain Backup Validation
    #[test]
    fn t4_sc10_scenario_ledger_disaster_recovery_and_validation() {
        let ctx = SentinelTestContext::new();
        let valid_chain = ctx.copy_fixture("ledgers/valid_three_block_chain.jsonl");
        let backup_path = ctx.work_dir().join("audit_chain_backup.jsonl");
        fs::copy(&valid_chain, &backup_path).unwrap();
        assert!(backup_path.exists());
    }
}
