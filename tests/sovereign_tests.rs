//! Comprehensive test suite for Milestone M1 (Sovereign Build Mode & Local-Only LLM Routing).

use needle::error::Error;
use needle::llm::{validate_loopback_url, LlmConfig, LoopbackValidator};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_loopback_validator_accepts_valid_loopback_addresses() {
    // Standard localhost string literals
    assert!(LoopbackValidator::validate_host("localhost").is_ok());
    assert!(LoopbackValidator::validate_host("127.0.0.1").is_ok());
    assert!(LoopbackValidator::validate_host("::1").is_ok());
    assert!(LoopbackValidator::validate_host("[::1]").is_ok());

    // Entire IPv4 127.0.0.0/8 loopback block
    assert!(LoopbackValidator::validate_host("127.0.0.2").is_ok());
    assert!(LoopbackValidator::validate_host("127.0.0.50").is_ok());
    assert!(LoopbackValidator::validate_host("127.1.2.3").is_ok());
    assert!(LoopbackValidator::validate_host("127.255.255.254").is_ok());

    // URLs with schemes and ports
    assert!(validate_loopback_url("http://127.0.0.1:11434").is_ok());
    assert!(validate_loopback_url("http://localhost:11434").is_ok());
    assert!(validate_loopback_url("https://127.0.0.1:8080/api/chat").is_ok());
    assert!(validate_loopback_url("http://[::1]:11434").is_ok());
}

#[test]
fn test_loopback_validator_rejects_non_loopback_ips() {
    // Private RFC 1918 addresses
    let res1 = LoopbackValidator::validate_host("192.168.1.1");
    assert!(matches!(res1.unwrap_err(), Error::OfflineStrictViolation(_)));

    let res2 = LoopbackValidator::validate_host("10.0.0.1");
    assert!(matches!(res2.unwrap_err(), Error::OfflineStrictViolation(_)));

    let res3 = LoopbackValidator::validate_host("172.16.0.1");
    assert!(matches!(res3.unwrap_err(), Error::OfflineStrictViolation(_)));

    // Wildcard address 0.0.0.0
    let res4 = LoopbackValidator::validate_host("0.0.0.0");
    assert!(matches!(res4.unwrap_err(), Error::OfflineStrictViolation(_)));

    // Public IPs
    let res5 = LoopbackValidator::validate_host("8.8.8.8");
    assert!(matches!(res5.unwrap_err(), Error::OfflineStrictViolation(_)));
}

#[test]
fn test_loopback_validator_rejects_remote_domains() {
    // Cloud provider domains
    let res1 = LoopbackValidator::validate_host("api.openai.com");
    assert!(matches!(res1.unwrap_err(), Error::OfflineStrictViolation(_)));

    let res2 = LoopbackValidator::validate_host("api.anthropic.com");
    assert!(matches!(res2.unwrap_err(), Error::OfflineStrictViolation(_)));

    let res3 = LoopbackValidator::validate_host("api.groq.com");
    assert!(matches!(res3.unwrap_err(), Error::OfflineStrictViolation(_)));

    let res4 = LoopbackValidator::validate_host("ollama.internal.corp");
    assert!(matches!(res4.unwrap_err(), Error::OfflineStrictViolation(_)));

    let res5 = LoopbackValidator::validate_host("example.com");
    assert!(matches!(res5.unwrap_err(), Error::OfflineStrictViolation(_)));
}

#[test]
fn test_extract_host_and_port() {
    let (h1, p1) = LoopbackValidator::extract_host_port("http://127.0.0.1:11434/api/tags", 11434);
    assert_eq!(h1, "127.0.0.1");
    assert_eq!(p1, 11434);

    let (h2, p2) = LoopbackValidator::extract_host_port("localhost:8080", 11434);
    assert_eq!(h2, "localhost");
    assert_eq!(p2, 8080);

    let (h3, p3) = LoopbackValidator::extract_host_port("127.0.0.1", 11434);
    assert_eq!(h3, "127.0.0.1");
    assert_eq!(p3, 11434);

    let (h4, p4) = LoopbackValidator::extract_host_port("", 11434);
    assert_eq!(h4, "127.0.0.1");
    assert_eq!(p4, 11434);
}

#[test]
fn test_llm_config_defaults_and_builder() {
    let config = LlmConfig::new("127.0.0.1", 11434, "llama3.2", true);
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 11434);
    assert_eq!(config.model, "llama3.2");
    assert!(config.offline_strict);

    let default_cfg = LlmConfig::default();
    assert!(!default_cfg.host.is_empty());
    assert!(default_cfg.port > 0);
    assert!(!default_cfg.model.is_empty());
}

#[test]
#[ignore = "doctor::run_diagnostics is a binary-only function; requires lib.rs exposure to test here"]
fn test_doctor_diagnostics_clean_ledger_state() {
    // Non-existent file verifies as clean 0-block state
    let _temp_nonexistent = std::path::PathBuf::from("non_existent_ledger_for_doctor_test.jsonl");
    // TODO: expose doctor diagnostics via needle library crate to enable this test
}

#[test]
#[ignore = "doctor::run_diagnostics is a binary-only function; requires lib.rs exposure to test here"]
fn test_doctor_diagnostics_tampered_ledger_localization() {
    // TODO: expose doctor diagnostics via needle library crate to enable this test
}

#[test]
#[ignore = "doctor::run_diagnostics is a binary-only function; requires lib.rs exposure to test here"]
fn test_doctor_diagnostics_remote_host_rejection() {
    // TODO: expose doctor diagnostics via needle library crate to enable this test
}
