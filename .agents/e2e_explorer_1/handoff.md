# Architectural Specification & E2E Test Infrastructure Report (NEEDLE-SENTINEL)

**Document**: E2E Test Infrastructure & Technical Specification  
**Agent**: `e2e_explorer_1`  
**Working Directory**: `d:\AEGIS_AST\.agents\e2e_explorer_1`  
**Target Specifications**: `d:\AEGIS_AST\TEST_INFRA.md`, `tests/fixtures/`, `tests/e2e_sentinel_tests.rs`  
**Date**: 2026-08-15  

---

## 1. Observation

Direct observations from codebase inspection, dependency audits, CLI entry points, and mined specifications:

1. **Codebase Baseline & Workspace Structure**:
   - `d:\AEGIS_AST\Cargo.toml`: Package `needle` (version 0.1.0, edition 2021) currently lacks `[features]` gating. All 30+ dependencies (`axum`, `sqlx`, `tower-cookies`, `reqwest`, `open`, `urlencoding`, `pdf-extract`, `tokio`, `tree-sitter`, `serde_json`, etc.) are linked unconditionally.
   - `src/lib.rs`: Exports core modules (`analysis`, `chunking`, `config`, `embedding`, `error`, `graph`, `indexing`, `llm`, `query`, `schema`, `server`, `storage`, `watcher`). Requires exports for `pub mod policy;` and `pub mod ledger;`.
   - `src/error.rs`: Defines enum `Error` with variants (`Io`, `InvalidPath`, `IndexNotFound`, `ChunkingError`, `EmbeddingError`, `IndexError`, `QueryError`, `ConfigError`, `SerializationError`, `Other`). Requires extensions `PolicyError(String)`, `LedgerError(String)`, and `OfflineStrictViolation(String)`.
   - `src/llm.rs`: `Provider` enum currently supports `Anthropic`, `OpenAI`, `Groq`, and `Ollama`. `LlmClient::from_env()` defaults to cloud providers when keys are present. In sovereign mode, must compile out cloud providers and route strictly to local Ollama (`127.0.0.1:11434`), enforcing `--offline-strict` loopback checks.
   - `src/main.rs` & `src/cli/`: Defines subcommands (`Init`, `Search`, `Status`, `Reindex`, `Config`, `Bench`, `Watch`, `Mcp`, `Serve`, `Report`, `Graph`). Requires subcommands `Doctor` (`--sovereign`), `Policy` (`ingest`, `list`), `Audit` (`--policy`, `--format`, `--output`, `--fail-on-violation`, `--sign-ledger`), and `Ledger` (`append`, `verify`, `keygen`).
   - `tests/`: Currently an empty directory awaiting the Sentinel E2E test suite.

2. **Cryptographic Primitives**:
   - `sha2 = "0.10"`: SHA-256 block and payload hashing.
   - `ed25519-dalek = { version = "2.1", features = ["rand_core"] }`: Ed25519 digital signature signing and verification.
   - `hex = "0.4"`: Hexadecimal encoding/decoding.

3. **Mandates & Critical Invariants**:
   - **Zero Panic / Unwrap on User Paths**: No `unwrap()`, `expect()`, or `panic!()` on user-supplied policy PDFs, Markdown/text files, ledger JSONL chains, or source code files.
   - **Zero Network in Sovereign Mode**: `cargo tree --no-default-features --features sovereign` must contain 0 networking crates (`axum`, `reqwest`, `hyper`, `sqlx`, `tower-cookies`, `tower-http`, `open`, `urlencoding`).
   - **Keypair Redaction**: Private keys must never be logged or formatted in `Debug`/`Display` (`"[REDACTED PRIVATE KEY]"`).
   - **Scanned PDF Loud Failure**: Scanned-image or text-free PDFs (<20 printable chars) must fail loudly with `Error::PolicyError`, not create empty documents.
   - **Fresh Chain Clean Verification**: Empty or missing `.needle/ledger/audit_chain.jsonl` must verify cleanly (returning 0 blocks, exit code 0).
   - **Exact Tamper Localization**: Modified payload, sequence gaps, corrupted hashes, or broken signatures must report the exact sequence number where corruption occurred.

---

## 2. Logic Chain

1. **Test Infrastructure Architecture**:
   - To guarantee complete verification without regressions, tests must be organized into 4 distinct tiers:
     - **Tier 1 (Feature Coverage)**: 5 deterministic tests for each feature F1–F20 (100 tests total), verifying positive happy-path execution and interface contracts.
     - **Tier 2 (Boundary & Corner Cases)**: 5 adversarial/negative tests for each feature F1–F20 (100 tests total), verifying edge cases, malformed inputs, missing files, empty states, corrupted data, and zero-panic error handling.
     - **Tier 3 (Cross-Feature Combinations)**: 20 pairwise/cross-module integration tests verifying interaction between features (e.g., Ingest -> Audit -> Graph -> Ledger Append -> Ledger Verify -> Tamper Localization).
     - **Tier 4 (Real-World Application Scenarios)**: 10 comprehensive multi-step end-to-end scenarios simulating real-world air-gapped security audits, CI/CD gates, ISO/NIST compliance certification, and multi-tenant ledger verification.
   - **Total Test Count**: 100 + 100 + 20 + 10 = **230 tests**.

2. **Fixture Strategy**:
   - Real-world and edge-case fixtures are isolated in `tests/fixtures/`:
     - Policies: Valid Markdown (`security_standard_v1.md`), valid text (`gdpr_data_privacy.txt`), custom syntax (`pci_dss_sample.policy`), valid PDF (`valid_nist_cybersecurity.pdf`), scanned image PDF (`scanned_image_only.pdf`), empty policy (`empty_policy.md`), whitespace only (`whitespace_only.txt`), malformed syntax (`malformed_clauses.md`).
     - Cryptographic Keys: Valid 32-byte Ed25519 private key (`test_auditor_ed25519.priv`), public key (`test_auditor_ed25519.pub`), secondary keypair (`secondary_auditor_ed25519.priv`), corrupted key (`corrupted_key.priv`).
     - Ledgers: Empty chain (`empty_chain.jsonl`), valid 3-block sequence (`valid_three_block_chain.jsonl`), tampered payload (`tampered_payload_seq1.jsonl`), sequence gap (`tampered_sequence_gap.jsonl`), invalid prev_hash (`tampered_prev_hash.jsonl`), invalid signature (`tampered_signature.jsonl`), deleted middle block (`tampered_deleted_block.jsonl`).
     - Sample Codebase: Mock repository (`tests/fixtures/sample_codebase/`) containing authentication, cryptography, storage, and networking code for compliance graph evaluation.

3. **Execution & Runner Harness**:
   - `SentinelTestContext`: Temporary sandbox managing isolated directories (`tempfile::TempDir`), fixture copying, isolated environment variables (`NEEDLE_HOME`, `OLLAMA_HOST`, etc.).
   - Process runners executing CLI commands via `std::process::Command` / `assert_cmd` capturing stdout, stderr, and exit codes.
   - Direct library unit and integration test harnesses asserting internal data structures, canonical serialization, and signature validation.

---

## 3. Full 4-Tier E2E Test Matrix (230 Tests)

### Tier 1: Feature Coverage Matrix (F1–F20, 5 Tests Each = 100 Tests)

| Test ID | Target Feature | Test Name | Input Conditions | Execution Command / API Call | Expected Assertions |
|---|---|---|---|---|---|
| **T1-F01-01** | F1 Sovereign Gating | `t1_f01_sovereign_cargo_flag_compiles` | `Cargo.toml` configured with `sovereign` feature | `cargo check --no-default-features --features sovereign` | Exit code 0, 0 compilation errors |
| **T1-F01-02** | F1 Sovereign Gating | `t1_f01_cloud_feature_flag_compiles` | `Cargo.toml` configured with default `cloud` feature | `cargo check --features cloud` | Exit code 0, compiles full web server & cloud modules |
| **T1-F01-03** | F1 Sovereign Gating | `t1_f01_sovereign_omits_axum_routes` | Sovereign build binary | `needle serve --port 7700` in sovereign build | Exits with informative error: `serve is disabled in sovereign build mode` |
| **T1-F01-04** | F1 Sovereign Gating | `t1_f01_sovereign_disables_oauth_module` | Sovereign build binary | Invoke internal `src/server/oauth.rs` cfg check | `cfg!(feature = "cloud") == false` |
| **T1-F01-05** | F1 Sovereign Gating | `t1_f01_sovereign_allows_local_mcp` | Sovereign build binary | `needle mcp` (stdio mode) | MCP server starts over stdio without opening network ports |
| **T1-F02-01** | F2 Doctor Sovereign | `t1_f02_doctor_sovereign_clean_pass` | Sovereign binary, valid local env | `needle doctor --sovereign` | Exit code 0, stdout contains `[PASS] Sovereign Mode: ACTIVE`, `Zero-Network Dependencies: PASS` |
| **T1-F02-02** | F2 Doctor Sovereign | `t1_f02_doctor_sovereign_json_output` | Sovereign binary, `--format json` | `needle doctor --sovereign --format json` | Exit code 0, valid JSON with `"sovereign_status": "pass"` |
| **T1-F02-03** | F2 Doctor Sovereign | `t1_f02_doctor_sovereign_audits_env_vars` | Clean env without cloud API keys | `needle doctor --sovereign` | Output confirms `Cloud API Keys: NONE DETECTED (PASS)` |
| **T1-F02-04** | F2 Doctor Sovereign | `t1_f02_doctor_sovereign_checks_ollama_loopback` | Mock/live Ollama at `127.0.0.1:11434` | `needle doctor --sovereign` | Output reports `Local LLM Endpoint: 127.0.0.1:11434 (PASS)` |
| **T1-F02-05** | F2 Doctor Sovereign | `t1_f02_doctor_sovereign_checks_ledger_state` | Initialized `.needle/ledger/` | `needle doctor --sovereign` | Output reports `Audit Ledger Status: READY (PASS)` |
| **T1-F03-01** | F3 Zero-Network Tree | `t1_f03_cargo_tree_no_reqwest` | Sovereign feature active | `cargo tree --no-default-features --features sovereign` | Output matches 0 instances of `reqwest` |
| **T1-F03-02** | F3 Zero-Network Tree | `t1_f03_cargo_tree_no_sqlx` | Sovereign feature active | `cargo tree --no-default-features --features sovereign` | Output matches 0 instances of `sqlx` |
| **T1-F03-03** | F3 Zero-Network Tree | `t1_f03_cargo_tree_no_axum` | Sovereign feature active | `cargo tree --no-default-features --features sovereign` | Output matches 0 instances of `axum` |
| **T1-F03-04** | F3 Zero-Network Tree | `t1_f03_cargo_tree_no_hyper` | Sovereign feature active | `cargo tree --no-default-features --features sovereign` | Output matches 0 instances of `hyper` |
| **T1-F03-05** | F3 Zero-Network Tree | `t1_f03_cargo_tree_no_tower_cookies` | Sovereign feature active | `cargo tree --no-default-features --features sovereign` | Output matches 0 instances of `tower-cookies` |
| **T1-F04-01** | F4 Default Compatibility | `t1_f04_default_cargo_build_succeeds` | Default features in `Cargo.toml` | `cargo build --release` | Exit code 0, binary produced at `target/release/needle` |
| **T1-F04-02** | F4 Default Compatibility | `t1_f04_default_runs_search_init` | Default binary, sample source dir | `needle init tests/fixtures/sample_codebase` | Exit code 0, builds index at `.needle/index/` |
| **T1-F04-03** | F4 Default Compatibility | `t1_f04_default_executes_hybrid_search` | Indexed sample codebase | `needle search "authenticate_user" --limit 5` | Exit code 0, returns matching AST symbol and file location |
| **T1-F04-04** | F4 Default Compatibility | `t1_f04_default_preserves_graph_command` | Indexed sample codebase | `needle graph --output test_graph.html` | Exit code 0, generates valid HTML graph |
| **T1-F04-05** | F4 Default Compatibility | `t1_f04_default_preserves_report_command` | Indexed sample codebase | `needle report --output test_report.md` | Exit code 0, generates valid Markdown report |
| **T1-F05-01** | F5 Sovereign LLM Routing | `t1_f05_sovereign_llm_routes_to_ollama` | Sovereign build, `LlmClient::from_env()` | `client.display_name()` | Returns `"Ollama/llama3.2"` or `"Ollama/..."` |
| **T1-F05-02** | F5 Sovereign LLM Routing | `t1_f05_sovereign_llm_ignores_anthropic_key` | Sovereign build, `ANTHROPIC_API_KEY=sk-ant-test` | `LlmClient::from_env()` | Provider remains `Ollama`, Anthropic provider compiled out |
| **T1-F05-03** | F5 Sovereign LLM Routing | `t1_f05_sovereign_llm_ignores_openai_key` | Sovereign build, `OPENAI_API_KEY=sk-test` | `LlmClient::from_env()` | Provider remains `Ollama`, OpenAI provider compiled out |
| **T1-F05-04** | F5 Sovereign LLM Routing | `t1_f05_sovereign_llm_uses_custom_model_env` | Sovereign build, `OLLAMA_MODEL=mistral:latest` | `LlmClient::from_env()` | Provider model matches `"mistral:latest"` |
| **T1-F05-05** | F5 Sovereign LLM Routing | `t1_f05_sovereign_llm_loopback_raw_tcp` | Sovereign build, local mock server at 11434 | `client.complete("sys", "user").await` | Successful HTTP/1.1 loopback chat completion |
| **T1-F06-01** | F6 Offline Strict | `t1_f06_offline_strict_accepts_localhost` | `--offline-strict`, target `http://127.0.0.1:11434` | `validate_loopback_url("http://127.0.0.1:11434")` | `Ok(())` |
| **T1-F06-02** | F6 Offline Strict | `t1_f06_offline_strict_accepts_ipv6_loopback` | `--offline-strict`, target `http://[::1]:11434` | `validate_loopback_url("http://[::1]:11434")` | `Ok(())` |
| **T1-F06-03** | F6 Offline Strict | `t1_f06_offline_strict_rejects_external_host` | `--offline-strict`, target `http://api.openai.com` | `validate_loopback_url("http://api.openai.com")` | Returns `Err(Error::OfflineStrictViolation(_))` |
| **T1-F06-04** | F6 Offline Strict | `t1_f06_offline_strict_rejects_lan_ip` | `--offline-strict`, target `http://192.168.1.100:11434` | `validate_loopback_url("http://192.168.1.100:11434")` | Returns `Err(Error::OfflineStrictViolation(_))` |
| **T1-F06-05** | F6 Offline Strict | `t1_f06_offline_strict_cli_flag_enforced` | `needle doctor --sovereign --offline-strict` | CLI execution with `--offline-strict` | Strict loopback validation reported in doctor audit |
| **T1-F07-01** | F7 Policy Parser | `t1_f07_parse_markdown_policy_success` | `tests/fixtures/policies/security_standard_v1.md` | `parse_policy_file(&path)` | `Ok(ExtractedDocument)` with non-empty text, title extracted |
| **T1-F07-02** | F7 Policy Parser | `t1_f07_parse_plaintext_policy_success` | `tests/fixtures/policies/gdpr_data_privacy.txt` | `parse_policy_file(&path)` | `Ok(ExtractedDocument)` with text containing GDPR clauses |
| **T1-F07-03** | F7 Policy Parser | `t1_f07_parse_custom_policy_extension` | `tests/fixtures/policies/pci_dss_sample.policy` | `parse_policy_file(&path)` | `Ok(ExtractedDocument)` parsed successfully |
| **T1-F07-04** | F7 Policy Parser | `t1_f07_parse_valid_pdf_policy` | `tests/fixtures/policies/valid_nist_cybersecurity.pdf` | `parse_policy_file(&path)` | `Ok(ExtractedDocument)` text extracted via `pdf-extract` |
| **T1-F07-05** | F7 Policy Parser | `t1_f07_policy_ingest_cli_command` | `needle policy ingest tests/fixtures/policies/security_standard_v1.md --name SecV1` | CLI execution | Exit code 0, outputs `Ingested policy 'SecV1': N clauses extracted` |
| **T1-F08-01** | F8 Scanned PDF Guard | `t1_f08_scanned_pdf_fails_loudly` | `tests/fixtures/policies/scanned_image_only.pdf` (0 text chars) | `parse_policy_file(&path)` | Returns `Err(Error::PolicyError(msg))` where msg contains `Scanned or image-only PDF detected` |
| **T1-F08-02** | F8 Scanned PDF Guard | `t1_f08_scanned_pdf_cli_non_zero_exit` | `needle policy ingest tests/fixtures/policies/scanned_image_only.pdf` | CLI execution | Exit code 1, stderr explains OCR required |
| **T1-F08-03** | F8 Scanned PDF Guard | `t1_f08_scanned_pdf_no_empty_doc_created` | Target `.needle/policy/` index | Check saved policies after failed ingest | Zero documents added to `.needle/policy/` |
| **T1-F08-04** | F8 Scanned PDF Guard | `t1_f08_scanned_pdf_char_count_in_error` | Scanned PDF with 5 stray OCR artifacts | `parse_policy_file(&path)` | Error message explicitly mentions `found 5 printable characters` |
| **T1-F08-05** | F8 Scanned PDF Guard | `t1_f08_scanned_pdf_no_panic_or_unwrap` | Scanned PDF input | `std::panic::catch_unwind(|| parse_policy_file(&path))` | Does not panic; returns structured `Err` |
| **T1-F09-01** | F9 Clause Structuring | `t1_f09_segment_markdown_headers` | Text with `# 1. Auth`, `## 1.1 Password` | `segment_clauses(text)` | Returns 2 `PolicyClause` structs with correct IDs |
| **T1-F09-02** | F9 Clause Structuring | `t1_f09_segment_article_section_format` | Text with `Article 1: Encryption. Section 1.1:...` | `segment_clauses(text)` | Correctly extracts clause numbers and titles |
| **T1-F09-03** | F9 Clause Structuring | `t1_f09_llm_obligation_structuring` | Raw clause + Mock LLM returning valid JSON | `structure_obligations_with_llm(&clause, &client).await` | Returns `Vec<PolicyObligation>` with `semantic_query`, `target_keywords` |
| **T1-F09-04** | F9 Clause Structuring | `t1_f09_rule_based_fallback_on_llm_failure` | Raw clause text: "Passwords MUST be hashed using bcrypt or argon2" | `structure_obligations_fallback(&clause)` | Extracts obligation with `ObligationType::Authentication`, `Severity::High` |
| **T1-F09-05** | F9 Clause Structuring | `t1_f09_obligation_id_generation` | 3 clauses ingested | Generated `PolicyObligation.id` | Follows deterministic scheme `POL-001`, `POL-002`, `POL-003` |
| **T1-F10-01** | F10 Policy-Code Matching | `t1_f10_match_obligation_to_auth_symbol` | Obligation: `Password Hashing`, indexed codebase | `evaluate_obligation_match(&ob, &query_engine, &graph)` | Matches `verify_password_hash` in `src/auth.rs` |
| **T1-F10-02** | F10 Policy-Code Matching | `t1_f10_match_obligation_to_crypto_symbol` | Obligation: `AES-GCM Encryption`, indexed codebase | `evaluate_obligation_match(&ob, &query_engine, &graph)` | Matches `encrypt_aes_gcm` in `src/crypto.rs` |
| **T1-F10-03** | F10 Policy-Code Matching | `t1_f10_match_unmapped_obligation` | Obligation: `Quantum Key Distribution` (no code) | `evaluate_obligation_match(&ob, &query_engine, &graph)` | Returns `ComplianceStatus::Unmapped`, confidence 1.0 |
| **T1-F10-04** | F10 Policy-Code Matching | `t1_f10_matcher_extracts_source_line_span` | Matched symbol `authenticate_user` | `link.line_start`, `link.line_end` | Non-zero line span matching AST node location |
| **T1-F10-05** | F10 Policy-Code Matching | `t1_f10_matcher_resolves_symbol_kind` | Matched AST node | `link.symbol_name` | Matches expected function/method identifier |
| **T1-F11-01** | F11 Compliance Graph | `t1_f11_construct_compliance_graph` | Ingested `PolicyDocument`, indexed `CodeGraph` | `evaluate_compliance(&doc, &qe, &cg)` | `Ok(PolicyComplianceGraph)` populated with links |
| **T1-F11-02** | F11 Compliance Graph | `t1_f11_graph_contains_governs_relation` | Generated `PolicyComplianceGraph` | Inspect links | Obligation links to governing code symbol |
| **T1-F11-03** | F11 Compliance Graph | `t1_f11_graph_computes_compliance_score` | 8 compliant links, 2 unmapped | `graph.calculate_score()` | Returns `80.0%` compliance score |
| **T1-F11-04** | F11 Compliance Graph | `t1_f11_graph_serializes_to_json` | `PolicyComplianceGraph` instance | `serde_json::to_string(&graph)` | `Ok(json_str)` with valid schema |
| **T1-F11-05** | F11 Compliance Graph | `t1_f11_graph_handles_empty_codebase` | Indexed empty codebase (0 files) | `evaluate_compliance(&doc, &qe, &cg)` | Returns all obligations as `ComplianceStatus::Unmapped` without error |
| **T1-F12-01** | F12 Audit CLI | `t1_f12_audit_console_format` | Ingested policy, indexed codebase | `needle audit --format console` | Exit code 0, stdout displays formatted table of compliance links |
| **T1-F12-02** | F12 Audit CLI | `t1_f12_audit_markdown_format` | `--format markdown --output audit.md` | `needle audit --format markdown --output audit.md` | Exit code 0, creates valid Markdown report file |
| **T1-F12-03** | F12 Audit CLI | `t1_f12_audit_json_format` | `--format json --output audit.json` | `needle audit --format json --output audit.json` | Exit code 0, valid JSON report with score and findings |
| **T1-F12-04** | F12 Audit CLI | `t1_f12_audit_filter_by_policy_id` | Two policies ingested (`SecV1`, `GDPR`) | `needle audit --policy SecV1` | Audit report only evaluates clauses from `SecV1` |
| **T1-F12-05** | F12 Audit CLI | `t1_f12_audit_fail_on_violation_flag` | Unmapped / non-compliant obligation present | `needle audit --fail-on-violation` | Exit code 1, reports detected compliance violations |
| **T1-F13-01** | F13 MCP Compliance Tools | `t1_f13_mcp_get_obligations_tool` | JSON-RPC call `{"method": "tools/call", "params": {"name": "get_obligations"}}` | MCP stdio dispatch | Returns JSON list of all extracted obligations |
| **T1-F13-02** | F13 MCP Compliance Tools | `t1_f13_mcp_get_obligations_filtered_by_severity` | Params: `{"name": "get_obligations", "arguments": {"severity": "critical"}}` | MCP stdio dispatch | Returns only critical severity obligations |
| **T1-F13-03** | F13 MCP Compliance Tools | `t1_f13_mcp_check_compliance_by_obligation` | Params: `{"name": "check_compliance", "arguments": {"obligation_id": "POL-001"}}` | MCP stdio dispatch | Returns compliance status and evidence for `POL-001` |
| **T1-F13-04** | F13 MCP Compliance Tools | `t1_f13_mcp_check_compliance_by_file` | Params: `{"name": "check_compliance", "arguments": {"file_path": "src/auth.rs"}}` | MCP stdio dispatch | Returns all obligations governing `src/auth.rs` |
| **T1-F13-05** | F13 MCP Compliance Tools | `t1_f13_mcp_get_compliance_report` | Params: `{"name": "get_compliance_report", "arguments": {"format": "json"}}` | MCP stdio dispatch | Returns full JSON compliance summary via MCP |
| **T1-F14-01** | F14 Canonical Encoding | `t1_f14_canonical_json_key_sorting` | JSON object `{"z": 1, "a": 2, "m": 3}` | `canonical_json_bytes(&val)` | Output bytes format keys alphabetically: `{"a":2,"m":3,"z":1}` |
| **T1-F14-02** | F14 Canonical Encoding | `t1_f14_canonical_json_no_whitespace` | JSON object with indentation/spaces | `canonical_json_bytes(&val)` | Output bytes contain zero redundant spaces or newlines |
| **T1-F14-03** | F14 Canonical Encoding | `t1_f14_canonical_json_nested_objects` | Nested object `{"b": {"y": 1, "x": 2}, "a": 0}` | `canonical_json_bytes(&val)` | Recursively sorted: `{"a":0,"b":{"x":2,"y":1}}` |
| **T1-F14-04** | F14 Canonical Encoding | `t1_f14_canonical_json_array_order_preserved` | Array `[3, 1, 2]` | `canonical_json_bytes(&val)` | Array element order preserved exactly: `[3,1,2]` |
| **T1-F14-05** | F14 Canonical Encoding | `t1_f14_canonical_json_deterministic_hash` | Two independently created values with same data | Compare `sha256(canonical_json(v1))` and `sha256(canonical_json(v2))` | Hashes are identical byte-for-byte |
| **T1-F15-01** | F15 SHA-256 Hashing | `t1_f15_sha256_known_vector` | String `"needle-sentinel"` | `sha256_hex(b"needle-sentinel")` | Matches 64-char lowercase hex SHA-256 standard vector |
| **T1-F15-02** | F15 SHA-256 Hashing | `t1_f15_payload_hash_computation` | Audit report JSON payload | `compute_payload_hash(&payload)` | Returns 64-char hex string |
| **T1-F15-03** | F15 SHA-256 Hashing | `t1_f15_signing_preimage_format` | Sequence 0, timestamp, prev_hash, type, payload_hash | `LedgerBlock::signing_preimage(...)` | Matches exact formatted string `{seq}:{ts}:{prev}:{type}:{hash}` |
| **T1-F15-04** | F15 SHA-256 Hashing | `t1_f15_block_hash_computation` | Full block fields | `compute_block_hash(&block)` | Matches SHA-256 of block preimage |
| **T1-F15-05** | F15 SHA-256 Hashing | `t1_f15_hash_chaining_integrity` | Block 0 and Block 1 | `block_1.prev_hash == block_0.block_hash` | Returns `true` |
| **T1-F16-01** | F16 Ed25519 Signatures | `t1_f16_keygen_generates_valid_pair` | `OsRng` entropy source | `LedgerKeypair::generate()` | Keypair contains 32-byte verifying key and signing key |
| **T1-F16-02** | F16 Ed25519 Signatures | `t1_f16_sign_and_verify_preimage` | Generated keypair, test preimage | `sign_preimage(&keypair, msg)` and `verify_signature(&pubkey, msg, sig)` | Returns `true` |
| **T1-F16-03** | F16 Ed25519 Signatures | `t1_f16_verify_rejects_wrong_pubkey` | Signed with Key A, verified with Key B | `verify_signature(&key_b_pub, msg, sig_a)` | Returns `false` |
| **T1-F16-04** | F16 Ed25519 Signatures | `t1_f16_verify_rejects_altered_message` | Preimage altered after signing | `verify_signature(&pubkey, altered_msg, sig)` | Returns `false` |
| **T1-F16-05** | F16 Ed25519 Signatures | `t1_f16_keypair_from_bytes_roundtrip` | 32 seed bytes | `LedgerKeypair::from_bytes(&seed)` | Produces deterministic, reproducible public key hex |
| **T1-F17-01** | F17 Key Redaction | `t1_f17_debug_fmt_redacts_private_key` | `LedgerKeypair` instance | `format!("{:?}", keypair)` | Contains `[REDACTED PRIVATE KEY]`; 0 private key bytes |
| **T1-F17-02** | F17 Key Redaction | `t1_f17_display_fmt_shows_only_pubkey` | `LedgerKeypair` instance | `format!("{}", keypair)` | Displays `LedgerKeypair(pubkey: <hex>)` |
| **T1-F17-03** | F17 Key Redaction | `t1_f17_tracing_log_redaction` | `tracing::debug!("Loaded key: {:?}", keypair)` | Captured log buffer | Log buffer contains `[REDACTED PRIVATE KEY]` |
| **T1-F17-04** | F17 Key Redaction | `t1_f17_error_display_no_key_leak` | Error involving key loading failure | `format!("{}", err)` | Contains file path only; 0 private key material |
| **T1-F17-05** | F17 Key Redaction | `t1_f17_key_file_permissions_unix` | `needle ledger keygen` on Unix | Inspect file mode of `key.priv` | Permissions set to `0600` (read/write user only) |
| **T1-F18-01** | F18 Append JSONL | `t1_f18_append_genesis_block` | Non-existent ledger, valid report payload | `append_to_ledger(&path, &keypair, EntryType::ComplianceAudit, payload)` | Appends line 1 with `sequence: 0`, `prev_hash: "0000...0000"` |
| **T1-F18-02** | F18 Append JSONL | `t1_f18_append_sequential_second_block` | Ledger with 1 block | `append_to_ledger(...)` | Appends line 2 with `sequence: 1`, `prev_hash == block_0.block_hash` |
| **T1-F18-03** | F18 Append JSONL | `t1_f18_append_cli_command` | `needle ledger append --report audit.json --gen-key-if-missing` | CLI execution | Exit code 0, outputs `Appended Block #0 (Hash: ...)` |
| **T1-F18-04** | F18 Append JSONL | `t1_f18_jsonl_formatting_one_line_per_block` | Ledger with 5 appended blocks | `std::fs::read_to_string(&ledger_path)` | Contains exactly 5 non-empty lines; each valid JSON |
| **T1-F18-05** | F18 Append JSONL | `t1_f18_append_preserves_existing_blocks` | Initial 3 blocks in ledger | Append 4th block | First 3 lines unchanged byte-for-byte; 4th line appended |
| **T1-F19-01** | F19 Fresh Chain Verify | `t1_f19_verify_non_existent_file` | Missing `.needle/ledger/audit_chain.jsonl` | `verify_ledger_file(&missing_path)` | `Ok(VerificationSummary { total_blocks: 0, is_valid: true, .. })` |
| **T1-F19-02** | F19 Fresh Chain Verify | `t1_f19_verify_zero_byte_file` | 0-byte file at ledger path | `verify_ledger_file(&empty_path)` | `Ok(VerificationSummary { total_blocks: 0, is_valid: true, .. })` |
| **T1-F19-03** | F19 Fresh Chain Verify | `t1_f19_verify_whitespace_only_file` | File containing only spaces and newlines | `verify_ledger_file(&ws_path)` | `Ok(VerificationSummary { total_blocks: 0, is_valid: true, .. })` |
| **T1-F19-04** | F19 Fresh Chain Verify | `t1_f19_verify_cli_fresh_chain_exit_zero` | Non-existent ledger path | `needle ledger verify` | Exit code 0, stdout: `Ledger verified: 0 blocks (empty chain)` |
| **T1-F19-05** | F19 Fresh Chain Verify | `t1_f19_verify_valid_single_block` | Valid genesis block appended | `verify_ledger_file(&ledger_path)` | `Ok(VerificationSummary { total_blocks: 1, is_valid: true, .. })` |
| **T1-F20-01** | F20 Tamper Localization | `t1_f20_tamper_payload_single_char` | Block 1 payload altered in 3-block chain | `verify_ledger_file(&tampered_path)` | `Err(Error::LedgerError(msg))` with `TAMPER DETECTED at sequence 1: payload_hash mismatch` |
| **T1-F20-02** | F20 Tamper Localization | `t1_f20_tamper_sequence_gap` | Sequence altered from 1 to 3 | `verify_ledger_file(&tampered_path)` | `Err(Error::LedgerError(msg))` with `TAMPER DETECTED at sequence 3: sequence discontinuity` |
| **T1-F20-03** | F20 Tamper Localization | `t1_f20_tamper_broken_prev_hash` | Block 2 prev_hash modified | `verify_ledger_file(&tampered_path)` | `Err(Error::LedgerError(msg))` with `TAMPER DETECTED at sequence 2: prev_hash mismatch` |
| **T1-F20-04** | F20 Tamper Localization | `t1_f20_tamper_signature_corruption` | Block 0 signature byte modified | `verify_ledger_file(&tampered_path)` | `Err(Error::LedgerError(msg))` with `TAMPER DETECTED at sequence 0: invalid Ed25519 signature` |
| **T1-F20-05** | F20 Tamper Localization | `t1_f20_tamper_cli_exit_code_and_output` | Tampered ledger file | `needle ledger verify --ledger tampered.jsonl` | Exit code 1, stderr outputs exact sequence number of tampering |

---

### Tier 2: Boundary & Corner Cases Matrix (F1–F20, 5 Tests Each = 100 Tests)

| Test ID | Target Feature | Test Name | Input Conditions | Execution Command / API Call | Expected Assertions |
|---|---|---|---|---|---|
| **T2-F01-01** | F1 Sovereign Gating | `t2_f01_no_features_specified_compiles_cloud` | `Cargo.toml` with default features | `cargo check` (default) | Compiles with `cloud` features enabled by default |
| **T2-F01-02** | F2 Sovereign Gating | `t2_f01_both_cloud_and_sovereign_conflict` | Mutually exclusive build check | `cargo check --features "cloud sovereign"` | Deterministic build behavior (either cleanly gates or priority resolves) |
| **T2-F01-03** | F1 Sovereign Gating | `t2_f01_sovereign_with_release_lto` | Release profile with LTO | `cargo build --release --no-default-features --features sovereign` | Successful build, optimized binary with zero dead network symbols |
| **T2-F01-04** | F1 Sovereign Gating | `t2_f01_sovereign_offline_crate_resolution` | `CARGO_NET_OFFLINE=true` | `cargo check --frozen --offline --no-default-features --features sovereign` | Compiles completely offline with pre-vendored/cached crates |
| **T2-F01-05** | F1 Sovereign Gating | `t2_f01_benchmarks_under_sovereign` | Sovereign build benchmarking | `cargo bench --no-default-features --features sovereign --no-run` | Benchmark harness compiles under sovereign mode |
| **T2-F02-01** | F2 Doctor Sovereign | `t2_f02_doctor_on_cloud_binary_fails` | Binary built with default `cloud` feature | `needle doctor --sovereign` | Exits with code 1, reports `[FAIL] Sovereign Mode: INACTIVE (cloud features detected)` |
| **T2-F02-02** | F2 Doctor Sovereign | `t2_f02_doctor_with_active_cloud_env_keys` | Env contains `ANTHROPIC_API_KEY`, `DATABASE_URL` | `needle doctor --sovereign` | Emits informational warning alerting operator of unneeded cloud keys |
| **T2-F02-03** | F2 Doctor Sovereign | `t2_f02_doctor_ollama_port_down` | Port 11434 closed/unbound | `needle doctor --sovereign` | Reports `[WARN/FAIL] Ollama offline at 127.0.0.1:11434`, exits cleanly without panic |
| **T2-F02-04** | F2 Doctor Sovereign | `t2_f02_doctor_unwritable_ledger_dir` | Read-only `.needle/ledger/` directory | `needle doctor --sovereign` | Reports `[FAIL] Ledger directory is not writable`, no panic |
| **T2-F02-05** | F2 Doctor Sovereign | `t2_f02_doctor_invalid_flag_handling` | Invalid CLI argument `needle doctor --invalid-flag` | `needle doctor --invalid-flag` | Clap derivation error, exit code 2, no panic |
| **T2-F03-01** | F3 Zero-Network Tree | `t2_f03_cargo_tree_no_tokio_net` | Sovereign dependency tree | `cargo tree --no-default-features --features sovereign` | Excludes optional network feature trees |
| **T2-F03-02** | F3 Zero-Network Tree | `t2_f03_cargo_tree_no_rustls` | Sovereign dependency tree | `cargo tree --no-default-features --features sovereign` | 0 instances of `rustls`, `native-tls`, `openssl` |
| **T2-F03-03** | F3 Zero-Network Tree | `t2_f03_cargo_tree_no_open_crate` | Sovereign dependency tree | `cargo tree --no-default-features --features sovereign` | 0 instances of `open` browser crate |
| **T2-F03-04** | F3 Zero-Network Tree | `t2_f03_cargo_tree_no_urlencoding` | Sovereign dependency tree | `cargo tree --no-default-features --features sovereign` | 0 instances of `urlencoding` crate |
| **T2-F03-05** | F3 Zero-Network Tree | `t2_f03_cargo_tree_all_targets_zero_net` | Sovereign tree on target x86_64-pc-windows-msvc | `cargo tree --target x86_64-pc-windows-msvc --no-default-features --features sovereign` | Zero networking crates across Windows target |
| **T2-F04-01** | F4 Default Compatibility | `t2_f04_default_serve_port_conflict` | Port 7700 already in use | `needle serve --port 7700` | Returns user-friendly error `Port 7700 in use`; exits cleanly |
| **T2-F04-02** | F4 Default Compatibility | `t2_f04_default_search_empty_query` | Empty query string `""` | `needle search ""` | Handles gracefully; returns empty list or usage prompt without panic |
| **T2-F04-03** | F4 Default Compatibility | `t2_f04_default_init_non_existent_dir` | Directory `d:\does_not_exist_xyz` | `needle init d:\does_not_exist_xyz` | Returns `Error::InvalidPath`; exits with clear message |
| **T2-F04-04** | F4 Default Compatibility | `t2_f04_default_init_permission_denied` | Restricted read permission folder | `needle init d:\restricted_folder` | Skips inaccessible files with warning; completes indexing rest |
| **T2-F04-05** | F4 Default Compatibility | `t2_f04_default_status_uninitialized_index` | Missing `.needle/` index | `needle status` | Reports `Index not initialized. Run needle init <dirs...>`; exit code 0 |
| **T2-F05-01** | F5 Sovereign LLM Routing | `t2_f05_ollama_unreachable_timeout` | Ollama port 11434 silent (hang) | `client.complete("sys", "user").await` with 2s timeout | Fails fast within 2s with explicit connection timeout error |
| **T2-F05-02** | F5 Sovereign LLM Routing | `t2_f05_ollama_returns_404_model_missing` | Ollama server returns HTTP 404 | `client.complete("sys", "user").await` | Error contains `Model '...' not found — run: ollama pull ...` |
| **T2-F05-03** | F5 Sovereign LLM Routing | `t2_f05_ollama_returns_500_internal_error` | Ollama server returns HTTP 500 | `client.complete("sys", "user").await` | Returns `Err(Ollama HTTP 500)` without crashing |
| **T2-F05-04** | F5 Sovereign LLM Routing | `t2_f05_ollama_returns_truncated_json` | Incomplete JSON chunk from Ollama | `client.complete("sys", "user").await` | Returns structured JSON parse error; no unwrap panic |
| **T2-F05-05** | F5 Sovereign LLM Routing | `t2_f05_ollama_empty_response_text` | Ollama returns `{"message": {"content": ""}}` | `client.complete("sys", "user").await` | Returns `Ok("")` cleanly |
| **T2-F06-01** | F6 Offline Strict | `t2_f06_offline_strict_rejects_dns_hostname` | `--offline-strict`, target `http://my-internal-server:11434` | `validate_loopback_url("http://my-internal-server:11434")` | Returns `Err(Error::OfflineStrictViolation(_))` |
| **T2-F06-02** | F6 Offline Strict | `t2_f06_offline_strict_rejects_public_ip` | `--offline-strict`, target `http://8.8.8.8:11434` | `validate_loopback_url("http://8.8.8.8:11434")` | Returns `Err(Error::OfflineStrictViolation(_))` |
| **T2-F06-03** | F6 Offline Strict | `t2_f06_offline_strict_rejects_hex_ip_obfuscation` | Target `http://0x7f000001:11434` | `validate_loopback_url("http://0x7f000001:11434")` | Canonicalized and verified strictly against `127.0.0.1` |
| **T2-F06-04** | F6 Offline Strict | `t2_f06_offline_strict_rejects_ftp_scheme` | Target `ftp://127.0.0.1:11434` | `validate_loopback_url("ftp://127.0.0.1:11434")` | Rejects non-HTTP loopback schemes |
| **T2-F06-05** | F6 Offline Strict | `t2_f06_offline_strict_empty_url_handling` | Target `""` | `validate_loopback_url("")` | Returns `Err(Error::InvalidPath(_))`; no panic |
| **T2-F07-01** | F7 Policy Parser | `t2_f07_parse_non_existent_file` | Missing file `d:\missing_policy.md` | `parse_policy_file(&path)` | Returns `Err(Error::InvalidPath(_))`; clear error message |
| **T2-F07-02** | F7 Policy Parser | `t2_f07_parse_unsupported_file_extension` | Unsupported file `policy.docx` | `parse_policy_file(&path)` | Returns `Err(Error::PolicyError(msg))` listing supported formats |
| **T2-F07-03** | F7 Policy Parser | `t2_f07_parse_zero_byte_markdown_file` | 0-byte file `empty_policy.md` | `parse_policy_file(&path)` | Returns `Err(Error::PolicyError("Policy file is empty"))` |
| **T2-F07-04** | F7 Policy Parser | `t2_f07_parse_corrupt_pdf_binary` | Non-PDF file renamed to `corrupt.pdf` | `parse_policy_file(&path)` | Returns `Err(Error::PolicyError(_))`; no unwrap panic |
| **T2-F07-05** | F7 Policy Parser | `t2_f07_parse_huge_policy_file` | 10MB policy text file | `parse_policy_file(&path)` | Safely reads and buffers text without memory exhaustion |
| **T2-F08-01** | F8 Scanned PDF Guard | `t2_f08_scanned_pdf_exact_19_chars` | Synthetic PDF with exactly 19 printable chars | `parse_policy_file(&path)` | Fails with scanned PDF error (threshold >= 20 chars) |
| **T2-F08-02** | F8 Scanned PDF Guard | `t2_f08_scanned_pdf_exact_20_chars` | Synthetic PDF with exactly 20 printable chars | `parse_policy_file(&path)` | Passes character threshold check |
| **T2-F08-03** | F8 Scanned PDF Guard | `t2_f08_scanned_pdf_whitespace_and_newlines_only` | PDF with 1000 spaces/newlines | `parse_policy_file(&path)` | Fails loudly (0 non-whitespace chars) |
| **T2-F08-04** | F8 Scanned PDF Guard | `t2_f08_scanned_pdf_null_bytes_only` | PDF containing control characters | `parse_policy_file(&path)` | Fails loudly; control chars excluded from printable count |
| **T2-F08-05** | F8 Scanned PDF Guard | `t2_f08_scanned_pdf_unicode_accents_counted` | PDF with accented letters (e.g. `Sécurité`) | `parse_policy_file(&path)` | Unicode letters correctly counted as printable characters |
| **T2-F09-01** | F9 Clause Structuring | `t2_f09_clause_with_no_normative_keywords` | Clause: "This document is for information only." | `structure_obligations_fallback(&clause)` | Returns empty obligation vector; no error or crash |
| **T2-F09-02** | F9 Clause Structuring | `t2_f09_llm_markdown_fence_stripping` | LLM returns ````json\n[{"title":"Auth"}]\n```` | `parse_llm_json_response(raw)` | Cleanly strips code fences and parses inner JSON array |
| **T2-F09-03** | F9 Clause Structuring | `t2_f09_llm_conversational_chatter_stripping` | LLM returns "Sure! Here is the JSON: [...] Hope that helps!" | `parse_llm_json_response(raw)` | Extracts bracketed JSON array cleanly |
| **T2-F09-04** | F9 Clause Structuring | `t2_f09_clause_with_10k_words` | Extremely long clause paragraph | `structure_obligations_fallback(&clause)` | Segments into sub-sentences safely without stack overflow |
| **T2-F09-05** | F9 Clause Structuring | `t2_f09_duplicate_clause_numbering` | Document with two `Section 1.1` headers | `segment_clauses(text)` | Disambiguates clause IDs (e.g. `CLAUSE-1.1-1`, `CLAUSE-1.1-2`) |
| **T2-F10-01** | F10 Policy-Code Matching | `t2_f10_matcher_unindexed_codebase_error` | Missing `.needle/index/` directory | `evaluate_compliance(&doc, &qe, &cg)` | Returns `Err(Error::IndexNotFound(_))`; no panic |
| **T2-F10-02** | F10 Policy-Code Matching | `t2_f10_matcher_special_characters_in_query` | Obligation query with quotes, slashes, regex | `QueryEngine::search(&query, 10, None)` | Sanitized and queried cleanly without tokenizer crash |
| **T2-F10-03** | F10 Policy-Code Matching | `t2_f10_matcher_binary_file_in_search_results` | Index contains binary chunk match | `matcher.rs` AST resolution | Skips non-source AST gracefully |
| **T2-F10-04** | F10 Policy-Code Matching | `t2_f10_matcher_zero_search_results` | Obscure query returning 0 results | `evaluate_obligation_match(...)` | Correctly creates `ComplianceStatus::Unmapped` link |
| **T2-F10-05** | F10 Policy-Code Matching | `t2_f10_matcher_low_confidence_threshold` | Search score < 0.2 | `matcher.rs` confidence scoring | Marks link as `ComplianceStatus::ManualReviewRequired` |
| **T2-F11-01** | F11 Compliance Graph | `t2_f11_graph_empty_policy_document` | `PolicyDocument` with 0 clauses | `evaluate_compliance(&empty_doc, &qe, &cg)` | Returns `PolicyComplianceGraph` with 0 links and 0.0% score |
| **T2-F11-02** | F11 Compliance Graph | `t2_f11_graph_all_unmapped_score_zero` | Document where 100% of obligations are unmapped | `graph.calculate_score()` | Returns `0.0%` |
| **T2-F11-03** | F11 Compliance Graph | `t2_f11_graph_all_compliant_score_hundred` | Document where 100% of obligations are compliant | `graph.calculate_score()` | Returns `100.0%` |
| **T2-F11-04** | F11 Compliance Graph | `t2_f11_graph_cyclic_ast_references` | CodeGraph with circular call graph edges | `evaluate_compliance(...)` | Traverses graph safely without infinite recursion |
| **T2-F11-05** | F11 Compliance Graph | `t2_f11_graph_concurrent_evaluation` | Multi-threaded compliance evaluation across 10 policies | `rayon::iter::ParallelIterator` | Thread-safe evaluation, zero data races |
| **T2-F12-01** | F12 Audit CLI | `t2_f12_audit_non_existent_policy_id` | `needle audit --policy NonExistentPolicy` | CLI execution | Exits with error: `Policy 'NonExistentPolicy' not found` |
| **T2-F12-02** | F12 Audit CLI | `t2_f12_audit_invalid_format_option` | `needle audit --format xml` | CLI execution | Clap error: invalid value 'xml' (expected console, markdown, json) |
| **T2-F12-03** | F12 Audit CLI | `t2_f12_audit_invalid_severity_threshold` | `needle audit --severity extreme` | CLI execution | Error: invalid severity (expected informational, low, medium, high, critical) |
| **T2-F12-04** | F12 Audit CLI | `t2_f12_audit_unwritable_output_path` | `needle audit --output /nonexistent_root/report.json` | CLI execution | Returns `Error::Io` with descriptive path message; exit code 1 |
| **T2-F12-05** | F12 Audit CLI | `t2_f12_audit_fail_on_violation_clean_pass` | 100% compliant codebase, `--fail-on-violation` | `needle audit --fail-on-violation` | Exit code 0 (zero violations detected) |
| **T2-F13-01** | F13 MCP Compliance Tools | `t2_f13_mcp_invalid_json_rpc_message` | Stdio input `{"malformed_json"` | MCP server dispatch | Returns JSON-RPC `ParseError (-32700)` |
| **T2-F13-02** | F13 MCP Compliance Tools | `t2_f13_mcp_unknown_tool_name` | Call tool `unknown_tool_xyz` | MCP server dispatch | Returns JSON-RPC `MethodNotFound (-32601)` |
| **T2-F13-03** | F13 MCP Compliance Tools | `t2_f13_mcp_missing_required_arguments` | Call `check_compliance` with empty params `{}` | MCP server dispatch | Returns JSON-RPC `InvalidParams (-32602)` |
| **T2-F13-04** | F13 MCP Compliance Tools | `t2_f13_mcp_check_compliance_non_existent_file` | File path `d:\missing_file.rs` | MCP server dispatch | Returns structured error indicating file not found |
| **T2-F13-05** | F13 MCP Compliance Tools | `t2_f13_mcp_get_obligations_empty_result` | Filter `severity: critical` when none exist | MCP server dispatch | Returns empty JSON array `[]` cleanly |
| **T2-F14-01** | F14 Canonical Encoding | `t2_f14_canonical_empty_json_object` | Value `json!({})` | `canonical_json_bytes(&val)` | Returns `b"{}"` |
| **T2-F14-02** | F24 Canonical Encoding | `t2_f14_canonical_empty_json_array` | Value `json!([])` | `canonical_json_bytes(&val)` | Returns `b"[]"` |
| **T2-F14-03** | F14 Canonical Encoding | `t2_f14_canonical_escaped_strings_in_payload` | Value `json!({"text": "line1\nline2\t\"quoted\""})` | `canonical_json_bytes(&val)` | Deterministic JSON escaping preserved |
| **T2-F14-04** | F14 Canonical Encoding | `t2_f14_canonical_unicode_nfc_normalization` | String with decomposed vs composed accents | `canonical_json_bytes(&val)` | Canonicalized to identical byte representation |
| **T2-F14-05** | F14 Canonical Encoding | `t2_f14_canonical_numeric_precision` | Value `json!({"count": 42, "ratio": 0.5})` | `canonical_json_bytes(&val)` | Deterministic float and integer formatting |
| **T2-F15-01** | F15 SHA-256 Hashing | `t2_f15_sha256_empty_byte_slice` | Byte slice `b""` | `sha256_hex(b"")` | Returns `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| **T2-F15-02** | F15 SHA-256 Hashing | `t2_f15_sha256_large_payload_10mb` | 10MB randomized byte buffer | `sha256_hex(&large_buffer)` | Completes within <50ms; produces valid 64-char hex |
| **T2-F15-03** | F15 SHA-256 Hashing | `t2_f15_preimage_with_colons_in_payload` | Payload hash containing colons | `LedgerBlock::signing_preimage(...)` | Preimage constructed unambiguously |
| **T2-F15-04** | F15 SHA-256 Hashing | `t2_f15_hash_case_sensitivity` | Hex output verification | `sha256_hex(...)` | All 64 characters are strictly lowercase `0-9a-f` |
| **T2-F15-05** | F15 SHA-256 Hashing | `t2_f15_genesis_prev_hash_is_64_zeros` | Constant check | `GENESIS_PREV_HASH` | Exactly 64 consecutive `'0'` characters |
| **T2-F16-01** | F16 Ed25519 Signatures | `t2_f16_verify_truncated_signature` | Signature string with only 64 hex chars (needs 128) | `verify_ed25519_signature(&pub, msg, &sig)` | Returns `false` or `Err(LedgerError::SignatureError)`; no panic |
| **T2-F16-02** | F16 Ed25519 Signatures | `t2_f16_verify_non_hex_characters_in_sig` | Signature with invalid characters `"ZZZZ..."` | `verify_ed25519_signature(...)` | Returns `false`; no panic |
| **T2-F16-03** | F16 Ed25519 Signatures | `t2_f16_verify_invalid_public_key_bytes` | Public key with invalid elliptic curve point | `LedgerKeypair::from_bytes(&invalid)` | Returns error / handles gracefully |
| **T2-F16-04** | F16 Ed25519 Signatures | `t2_f16_sign_empty_message` | Empty message `b""` | `sign_preimage(&keypair, b"")` | Generates valid 128-char signature verifying against `b""` |
| **T2-F16-05** | F16 Ed25519 Signatures | `t2_f16_verify_with_mismatched_key_length` | Public key string with 30 hex chars | `verify_ed25519_signature(...)` | Returns `false` cleanly |
| **T2-F17-01** | F17 Key Redaction | `t2_f17_panic_payload_redacts_private_key` | Intentional test panic with keypair in scope | `std::panic::catch_unwind(...)` | Panic message does not contain private key bytes |
| **T2-F17-02** | F17 Key Redaction | `t2_f17_json_serialization_omits_private_key` | `serde_json::to_string(&keypair)` | Serialized string | Signing key is not serialized or marked `skip_serializing` |
| **T2-F17-03** | F17 Key Redaction | `t2_f17_keypair_clone_redaction_maintained` | `keypair.clone()` | `format!("{:?}", cloned)` | Cloned instance preserves redacted Debug representation |
| **T2-F17-04** | F17 Key Redaction | `t2_f17_keygen_cli_existing_key_no_overwrite` | Existing key in `.needle/ledger/` | `needle ledger keygen` without `--force` | Fails with `Key already exists. Use --force to overwrite` |
| **T2-F17-05** | F17 Key Redaction | `t2_f17_keygen_cli_force_overwrites_cleanly` | Existing key, `--force` flag | `needle ledger keygen --force` | Overwrites key files; outputs new public key hex |
| **T2-F18-01** | F18 Append JSONL | `t2_f18_append_to_read_only_file` | Read-only permissions on ledger file | `append_to_ledger(...)` | Returns `Err(Error::Io(_))`; no data loss or corruption |
| **T2-F18-02** | F18 Append JSONL | `t2_f18_append_with_missing_key_file` | Private key file not found | `needle ledger append --report r.json` (no gen flag) | Returns `LedgerError::KeyNotFound` with actionable prompt |
| **T2-F18-03** | F18 Append JSONL | `t2_f18_append_huge_100k_blocks_performance` | Append 1,000 blocks in succession | `for _ in 0..1000 { append_to_ledger(...) }` | Append time remains O(1) constant time per block (<5ms) |
| **T2-F18-04** | F18 Append JSONL | `t2_f18_append_concurrent_file_lock` | Two threads appending simultaneously | Multi-threaded append test | File lock prevents interleaved corrupted JSON lines |
| **T2-F18-05** | F18 Append JSONL | `t2_f18_append_invalid_json_report_file` | Report file containing invalid JSON | `needle ledger append --report corrupt.json` | Fails before appending; ledger file remains pristine |
| **T2-F19-01** | F19 Fresh Chain Verify | `t2_f19_verify_ledger_is_a_directory` | Directory path passed to `--ledger` | `verify_ledger_file(&dir_path)` | Returns `Err(Error::Io(_))` with descriptive error |
| **T2-F19-02** | F19 Fresh Chain Verify | `t2_f19_verify_unreadable_ledger_file` | Ledger file without read permissions | `verify_ledger_file(&unreadable)` | Returns `Err(Error::Io(_))`; exits with code 1 |
| **T2-F19-03** | F19 Fresh Chain Verify | `t2_f19_verify_trailing_newlines_in_ledger` | Ledger file with 10 trailing blank lines | `verify_ledger_file(&path)` | Filters empty lines; verifies blocks accurately |
| **T2-F19-04** | F19 Fresh Chain Verify | `t2_f19_verify_single_empty_line_in_ledger` | Ledger with 1 newline character `\n` | `verify_ledger_file(&path)` | Returns `Ok(0 blocks)` cleanly |
| **T2-F19-05** | F19 Fresh Chain Verify | `t2_f19_verify_carriage_return_line_endings` | Ledger with Windows CRLF `\r\n` | `verify_ledger_file(&crlf_path)` | Parses lines cleanly on both Windows and Unix |
| **T2-F20-01** | F20 Tamper Localization | `t2_f20_tamper_middle_block_deletion` | 5-block chain with block 2 deleted | `verify_ledger_file(&path)` | Detects sequence discontinuity / hash mismatch at sequence 2 |
| **T2-F20-02** | F20 Tamper Localization | `t2_f20_tamper_genesis_prev_hash_non_zero` | Block 0 has `prev_hash: "abcd..."` | `verify_ledger_file(&path)` | Fails at sequence 0 with `prev_hash mismatch (expected 0000...0000)` |
| **T2-F20-03** | F20 Tamper Localization | `t2_f20_tamper_reordered_blocks` | Blocks 1 and 2 swapped in order | `verify_ledger_file(&path)` | Fails at sequence 2 with sequence discontinuity |
| **T2-F20-04** | F20 Tamper Localization | `t2_f20_tamper_timestamp_modified` | Block 1 timestamp altered by 1 second | `verify_ledger_file(&path)` | Fails at sequence 1 with `invalid Ed25519 signature` |
| **T2-F20-05** | F20 Tamper Localization | `t2_f20_tamper_truncated_json_line` | Block 2 JSON line cut off mid-payload | `verify_ledger_file(&path)` | Fails at sequence 2 with `invalid JSON block structure` |

---

### Tier 3: Cross-Feature Integration Combinations (20+ Tests)

| Test ID | Features Combined | Test Name | Input Conditions & Workflow | Expected Assertions |
|---|---|---|---|---|
| **T3-X01** | F1 + F2 + F3 | `t3_sovereign_build_doctor_and_tree_guarantee` | Build sovereign binary, run `cargo tree`, execute `needle doctor --sovereign` | 0 networking crates in tree; doctor reports `[PASS]` for all sovereign checks |
| **T3-X02** | F1 + F5 + F6 | `t3_sovereign_llm_offline_strict_loopback` | Sovereign build with `--offline-strict`, mock Ollama at `127.0.0.1:11434` | All completions route to loopback; remote endpoints rejected with `OfflineStrictViolation` |
| **T3-X03** | F7 + F8 + F12 | `t3_policy_ingest_scanned_guard_and_audit` | Attempt ingest of `scanned_image_only.pdf`, then ingest `security_standard_v1.md`, run `needle audit` | Ingest 1 fails loudly (exit code 1); Ingest 2 succeeds; Audit evaluates only Ingest 2 |
| **T3-X04** | F7 + F9 + F10 + F11 | `t3_policy_to_compliance_graph_pipeline` | Ingest Markdown policy -> Structure obligations -> QueryEngine search -> CodeGraph AST resolution | Generates complete `PolicyComplianceGraph` with `Governs` and `Implements` links |
| **T3-X05** | F10 + F11 + F12 | `t3_audit_cli_generates_json_and_markdown` | Run `needle audit --format json --output a.json` and `needle audit --format markdown --output a.md` | Both files created; JSON matches report schema; Markdown contains readable tables |
| **T3-X06** | F12 + F14 + F15 + F16 + F18 | `t3_audit_sign_ledger_end_to_end` | `needle audit --sign-ledger` | Audit runs, serializes report canonically, SHA-256 hashes, signs Ed25519, appends Block #0 to ledger |
| **T3-X07** | F18 + F19 + F20 | `t3_ledger_append_verify_and_tamper_cycle` | Verify empty -> Append 3 blocks -> Verify valid -> Modify byte in Block 1 -> Verify fails at Seq 1 | Verify 1: 0 blocks; Verify 2: 3 blocks valid; Verify 3: `TAMPER DETECTED at sequence 1` |
| **T3-X08** | F16 + F17 + F18 | `t3_keygen_redaction_and_append` | `needle ledger keygen` -> Log keypair with debug -> Append block with key | Debug log masks private key; block signed and appended successfully |
| **T3-X09** | F9 + F10 + F13 | `t3_mcp_obligations_and_compliance_check` | Ingest policy, index codebase -> Call MCP `get_obligations` -> Call MCP `check_compliance` | MCP returns extracted obligations and matching AST code evidence snippets |
| **T3-X10** | F4 + F7 + F12 | `t3_default_build_policy_and_search_coexistence` | Ingest policy -> Run hybrid search `needle search` -> Run `needle audit` | Standard search and policy audit both operate seamlessly on shared `.needle/` store |
| **T3-X11** | F5 + F9 + F6 | `t3_offline_strict_structurer_fallback` | `--offline-strict` with Ollama down -> Ingest policy with normative words | Structurer falls back cleanly to heuristic rule extractor without throwing network error |
| **T3-X12** | F11 + F12 + F20 | `t3_audit_fail_on_violation_with_ledger_signing` | Audit with violations present and `--fail-on-violation --sign-ledger` | Appends audit block to ledger, then exits with code 1; ledger record is intact and verifiable |
| **T3-X13** | F14 + F15 + F16 | `t3_block_hashing_and_signature_consistency` | Direct API: Create block, compute canonical hash, sign preimage, verify block hash | All hashes match recomputed values; signature verifies against public key |
| **T3-X14** | F18 + F20 | `t3_ledger_multiple_append_and_middle_tamper` | Append 10 blocks -> Alter payload in Block 7 -> Run `needle ledger verify` | Verifier halts at sequence 7; blocks 0–6 pass; blocks 8–9 flagged invalid chain |
| **T3-X15** | F7 + F9 + F12 | `t3_multi_format_policy_audit` | Ingest `.pdf`, `.md`, and `.policy` files -> Run `needle audit` | Audit unifies obligations from all 3 formats into a single comprehensive report |
| **T3-X16** | F2 + F6 + F18 | `t3_doctor_verifies_tampered_ledger` | Corrupt block in ledger -> Run `needle doctor --sovereign` | Doctor report marks `[FAIL] Audit Ledger Integrity: TAMPER DETECTED at sequence N` |
| **T3-X17** | F13 + F11 | `t3_mcp_compliance_report_matches_cli_json` | Execute CLI `needle audit --format json` and MCP `get_compliance_report` | Compliance score, mapped symbols, and obligation counts match identically |
| **T3-X18** | F1 + F16 + F18 | `t3_sovereign_ledger_append_and_verify` | Sovereign binary: Append audit report -> Verify ledger | Appends and verifies without loading any external network dependencies |
| **T3-X19** | F8 + F7 + F9 | `t3_batch_ingest_with_one_scanned_pdf` | Ingest directory with 3 valid Markdown policies and 1 scanned PDF | Valid policies ingested; scanned PDF skipped with clear warning/error; 0 data corruption |
| **T3-X20** | F15 + F16 + F20 | `t3_unauthorized_key_tamper_detection` | Sign Block 2 with unauthorized secondary keypair | `verify_ledger_file` fails at sequence 2 with `invalid Ed25519 signature` |

---

### Tier 4: Real-World Application Scenarios (10 Scenarios)

| Scenario ID | Scenario Name | Real-World Context & Multi-Step Workflow | Target Assertions & Verification |
|---|---|---|---|
| **T4-SC01** | `scenario_air_gapped_defense_audit` | **Air-Gapped Defense Codebase Certification**: <br>1. Verify system in sovereign mode (`needle doctor --sovereign --offline-strict`).<br>2. Index air-gapped classified repository.<br>3. Ingest DoD / NIST 800-53 security policy (`valid_nist_cybersecurity.pdf`).<br>4. Run `needle audit --fail-on-violation --format markdown --output nist_audit.md --sign-ledger`.<br>5. Verify cryptographic audit chain (`needle ledger verify`). | Zero network packets transmitted; audit links crypto/auth AST symbols; ledger contains signed block 0; exit code 0. |
| **T4-SC02** | `scenario_ci_cd_compliance_gate` | **Automated CI/CD Quality & Compliance Gate**: <br>1. Ingest corporate secure coding policy.<br>2. Build and index PR source tree.<br>3. Run `needle audit --fail-on-violation --format json --output pr_audit.json`.<br>4. Assert CI pass when compliance score >= 90%.<br>5. Introduce hardcoded unhashed password violation in `auth.rs`.<br>6. Re-run `needle audit --fail-on-violation`. | Step 4 passes (exit code 0); Step 6 fails build (exit code 1) with explicit report of unmapped/violating obligation. |
| **T4-SC03** | `scenario_adversarial_ledger_tampering_investigation` | **Forensic Audit Chain Tamper Investigation**: <br>1. Keygen auditor keypair.<br>2. Append 5 successive release audit snapshots.<br>3. Verify valid 5-block chain.<br>4. Adversary alters CVE severity payload in Block 2.<br>5. Auditor runs `needle ledger verify --verbose`. | Verifier pinpoints exact breach: `TAMPER DETECTED at sequence 2: payload_hash mismatch`, proving non-repudiation. |
| **T4-SC04** | `scenario_multi_standard_governance` | **Multi-Standard Regulatory Governance (SOC2 + GDPR + PCI-DSS)**: <br>1. Ingest `soc2_security.md`, `gdpr_privacy.txt`, and `pci_dss.policy`.<br>2. Execute codebase compliance mapping.<br>3. Query individual compliance scores per standard.<br>4. Generate unified governance report.<br>5. Record multi-standard snapshot to cryptographic ledger. | All three policy standards parsed; AST symbols mapped across access control, encryption, retention; report generated cleanly. |
| **T4-SC05** | `scenario_scanned_pdf_quarantine` | **Policy Ingestion Pipeline Scanned Document Quarantine**: <br>1. Batch ingest policy archive containing mix of digital PDFs, Markdown specs, and scanned image PDFs.<br>2. Scanned image PDF is trapped by guard.<br>3. Pipeline emits quarantine alert with exact char count.<br>4. Valid policies proceed to structuring and indexing without failure. | Scanned PDF rejected with `Error::PolicyError`; valid documents indexed; zero empty/ghost documents in store. |
| **T4-SC06** | `scenario_offline_llm_graceful_degradation` | **Zero-Connectivity Graceful Fallback**: <br>1. Start Needle in `--offline-strict` sovereign mode.<br>2. Simulate local Ollama process restart / outage.<br>3. Run policy ingestion and compliance evaluation.<br>4. Engine detects offline Ollama and engages deterministic rule-based obligation structuring and BM25 AST matching. | Zero network attempts; ingestion completes successfully via heuristic fallback; audit report generated. |
| **T4-SC07** | `scenario_mcp_ai_agent_compliance_review` | **Autonomous AI Security Agent MCP Integration**: <br>1. Spawn `needle mcp` server over stdio.<br>2. Agent calls `get_obligations` to discover compliance requirements.<br>3. Agent calls `check_compliance` on `src/crypto.rs`.<br>4. Agent receives AST node references, line numbers, and evidence snippets. | JSON-RPC stdio protocol executes cleanly; agent receives structured evidence without requiring direct filesystem access. |
| **T4-SC08** | `scenario_cryptographic_key_rotation` | **Auditor Key Rotation & Chain Continuity**: <br>1. Append 3 blocks signed with Auditor Key A.<br>2. Rotate to Auditor Key B using `needle ledger keygen`.<br>3. Append 2 blocks signed with Auditor Key B.<br>4. Run `needle ledger verify`. | Verifier validates blocks 0–2 against Key A and blocks 3–4 against Key B; entire chain passes continuity checks. |
| **T4-SC09** | `scenario_large_enterprise_monorepo_audit` | **Enterprise Monorepo Scale Audit**: <br>1. Index large codebase (10,000+ AST symbols across Rust, Python, Go, TypeScript).<br>2. Ingest 50-clause enterprise compliance policy.<br>3. Execute compliance evaluation with multi-threaded matching.<br>4. Generate audit report within performance budget (<5s). | Memory usage remains stable (<200MB); all 50 clauses matched; report accurately reflects symbol links across all languages. |
| **T4-SC10** | `scenario_ledger_disaster_recovery_and_validation` | **Disaster Recovery & Chain Backup Validation**: <br>1. Create 10-block ledger with signed audit history.<br>2. Take cold storage backup of `audit_chain.jsonl`.<br>3. Simulate live storage corruption / bit rot on Block 8.<br>4. Run `needle ledger verify` to locate damaged block.<br>5. Restore backup from cold storage and re-verify. | Step 4 identifies `TAMPER DETECTED at sequence 8`; Step 5 restores cleanly with 10 blocks verified. |

---

## 4. Test Fixtures Specification (`tests/fixtures/`)

### Fixture Directory Layout
```
tests/fixtures/
├── policies/
│   ├── security_standard_v1.md       # Valid Markdown policy (Auth, Encryption, Logging)
│   ├── gdpr_data_privacy.txt         # Valid plaintext policy (Retention, Sanitization)
│   ├── pci_dss_sample.policy         # Custom policy syntax (SecretMgmt, NetworkIsolation)
│   ├── valid_nist_cybersecurity.pdf  # Valid binary PDF with extractable text streams
│   ├── scanned_image_only.pdf        # Scanned image-only PDF (0 extractable text characters)
│   ├── empty_policy.md               # 0-byte empty file
│   ├── whitespace_only.txt           # File containing only spaces, tabs, and newlines
│   └── malformed_clauses.md          # Corrupt headers, broken UTF-8 sequences
├── keys/
│   ├── test_auditor_ed25519.priv     # 32-byte Ed25519 private key seed (Hex: 9d61b19de...)
│   ├── test_auditor_ed25519.pub      # 32-byte Ed25519 public key (Hex: d75a980182b10ab7d...)
│   ├── secondary_auditor.priv        # Secondary private key for unauthorized signing tests
│   ├── secondary_auditor.pub         # Secondary public key
│   └── corrupted_key.priv            # Truncated 16-byte invalid key file
├── ledgers/
│   ├── empty_chain.jsonl             # 0-byte file for fresh chain verification
│   ├── valid_three_block_chain.jsonl # 3 valid sequentially chained & signed blocks
│   ├── tampered_payload_seq1.jsonl   # Block 1 payload modified by 1 character
│   ├── tampered_sequence_gap.jsonl   # Sequence numbers skip from 0 to 2
│   ├── tampered_prev_hash.jsonl      # Block 2 prev_hash deliberately corrupted
│   ├── tampered_signature.jsonl      # Block 0 Ed25519 signature corrupted
│   └── tampered_deleted_block.jsonl  # Block 1 removed from a 3-block chain
└── sample_codebase/
    ├── Cargo.toml
    └── src/
        ├── auth.rs                   # Functions: authenticate_user, verify_password_hash, issue_jwt
        ├── crypto.rs                 # Functions: encrypt_aes_gcm, decrypt_aes_gcm, generate_salt
        ├── storage.rs                # Functions: store_user_record, purge_expired_records
        └── network.rs                # Functions: send_telemetry, fetch_remote_data
```

---

## 5. Test Suite Architecture (`tests/e2e_sentinel_tests.rs`)

### Helper Harness & Execution Architecture

```rust
//! NEEDLE-SENTINEL Comprehensive E2E Test Suite
//! Implements Tiers 1 through 4 (230 tests total)

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

/// Isolated test sandbox environment
pub struct SentinelTestContext {
    pub temp_dir: TempDir,
    pub needle_bin: PathBuf,
    pub fixtures_dir: PathBuf,
}

impl SentinelTestContext {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temporary test directory");
        let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let needle_bin = PathBuf::from(env!("CARGO_BIN_EXE_needle"));

        Self {
            temp_dir,
            needle_bin,
            fixtures_dir,
        }
    }

    pub fn work_dir(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Execute needle CLI in isolated sandbox
    pub fn run_cmd(&self, args: &[&str]) -> Output {
        Command::new(&self.needle_bin)
            .current_dir(self.work_dir())
            .args(args)
            .env("NEEDLE_HOME", self.work_dir())
            .env("RUST_BACKTRACE", "1")
            .output()
            .expect("Failed to execute needle binary")
    }

    /// Copy fixture to sandbox
    pub fn copy_fixture(&self, relative_path: &str) -> PathBuf {
        let src = self.fixtures_dir.join(relative_path);
        let dest = self.work_dir().join(relative_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::copy(&src, &dest).expect("Failed to copy fixture");
        dest
    }
}

// ── Assertion Helpers ─────────────────────────────────────────────────────────

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
```

---

## 6. Caveats

1. **Local Ollama Dependency**: Tests interacting with live LLM completions require Ollama running at `127.0.0.1:11434`. All E2E tests are designed with mock loopback handlers and deterministic rule fallbacks so that the entire 230-test suite runs hermetically in offline CI environments without external daemons.
2. **Binary Path Resolution**: `env!("CARGO_BIN_EXE_needle")` requires the test harness to be invoked via `cargo test --test e2e_sentinel_tests`.
3. **Windows File Path Escaping**: File paths in JSON payloads and CLI arguments use `Path::to_string_lossy()` or forward slashes to ensure cross-platform compatibility across Windows and Linux.

---

## 7. Conclusion

The specification defines an exhaustive, rigorous 4-Tier E2E test infrastructure for NEEDLE-SENTINEL:
1. **Tier 1 (100 Feature Coverage Tests)**: Complete functional verification of F1–F20.
2. **Tier 2 (100 Boundary & Corner Tests)**: Rigorous negative testing, scanned PDF trapping, zero unwrap/panic enforcement, and tamper detection.
3. **Tier 3 (20+ Cross-Feature Tests)**: Pairwise and multi-module integration guarantees.
4. **Tier 4 (10 Real-World Scenarios)**: End-to-end air-gapped defense auditing, CI/CD gates, and tamper forensics.
5. **Fixtures & Architecture**: Deterministic fixtures in `tests/fixtures/` and isolated test execution context in `tests/e2e_sentinel_tests.rs`.

---

## 8. Verification Method

To verify the test suite once implemented:
1. **Run Full E2E Test Suite**:
   ```bash
   cargo test --test e2e_sentinel_tests
   ```
2. **Run Under Sovereign Feature Set**:
   ```bash
   cargo test --test e2e_sentinel_tests --no-default-features --features sovereign
   ```
3. **Verify Zero Network in Sovereign Mode**:
   ```bash
   cargo tree --no-default-features --features sovereign | Select-String "reqwest|hyper|sqlx|axum|tower"
   # Assert empty output
   ```
4. **Check Clippy Compliance**:
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```
