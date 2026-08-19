# Project: NEEDLE-SENTINEL

## Architecture
NEEDLE-SENTINEL extends the Needle code intelligence platform for air-gapped AST-level code auditing, policy compliance verification, cryptographic audit ledger tracking, and sovereign local-only LLM routing.

```
+-----------------------------------------------------------------------------------+
|                                 CLI Entry Points                                   |
|   needle doctor --sovereign  |  needle audit  |  needle ledger  |  needle policy   |
+-----------------------------------------------------------------------------------+
|                                   MCP Server                                      |
|    get_obligations    |    check_compliance    |    get_compliance_report          |
|    verify_ledger      |    sign_ledger         |    get_ledger_status              |
+------------------------------------+----------------------------------------------+
                                     |
           +-------------------------+-------------------------+
           |                                                   |
           v                                                   v
+-----------------------+                           +-----------------------+
|    Policy Subsystem   |                           |    Ledger Subsystem   |
|     (src/policy/)     |                           |     (src/ledger/)     |
| - parser (pdf/txt/md) |                           | - block (canonical)   |
| - scanned PDF guard   |                           | - crypto (sha2/ed255) |
| - clause segmentation |                           | - keypair (redacted)  |
| - LLM / rule structure|                           | - append-only writer  |
| - matcher & graph     |                           | - verifier (tamper)   |
+-----------+-----------+                           +-----------+-----------+
            |                                                   |
            v                                                   |
+-----------------------+                                       |
|  Core Engine & Graph  |                                       |
| - QueryEngine (search)|                                       |
| - CodeGraph (AST nodes|                                       |
+-----------+-----------+                                       |
            |                                                   |
            v                                                   v
+-----------------------+                           +-----------------------+
|  Local LLM / Ollama   |                           |  Audit Trail Storage  |
| (src/llm.rs + loopback|                           | (.needle/ledger/      |
|  --offline-strict)    |                           |  audit_chain.jsonl)   |
+-----------------------+                           +-----------------------+
```

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| F1 | `[features]` Sovereign Build Gating | Feature flag `--features sovereign` in `Cargo.toml` gating `axum`, `sqlx`, `reqwest`, `open`, `tower-*` under `cloud` | M1 | R1 Spec |
| F2 | `needle doctor --sovereign` | Diagnostic CLI command verifying absence of cloud routes, zero networking dependencies, and local readiness | M1 | R1 Spec |
| F3 | Zero-Network Dependency Guarantee | `cargo tree --no-default-features --features sovereign` contains 0 networking crates | M1 | R1 Spec / AC |
| F4 | Default Build Backward Compatibility | `cargo build --release` preserves all default cloud features and behavior | M1 | R1 Spec / AC |
| F5 | Sovereign Local-Only LLM Routing | `src/llm.rs` compiles out external cloud providers and routes exclusively to local Ollama | M1 | R2 Spec |
| F6 | Runtime `--offline-strict` Enforcement | Flag rejecting all non-loopback network calls and prohibiting silent fallbacks | M1 | R2 Spec |
| F7 | PDF & Text Policy Parser | Parser in `src/policy/parser.rs` extracting text from `.pdf`, `.md`, `.txt`, `.policy` | M2 | R3 Spec |
| F8 | Scanned-Image PDF Guard | Explicit loud error on scanned/image-only PDFs (<20 printable chars), preventing silent empty indexing | M2 | R3 Spec / AC |
| F9 | Clause & Obligation Structuring | Segment clauses and structure obligations via LLM with deterministic rule fallback | M2 | R3 Spec |
| F10 | Policy-Code Matching Engine | Hybrid AST search via `QueryEngine` mapping obligations to `CodeGraph` AST symbols | M3 | R3 Spec |
| F11 | Policy Compliance Graph | Graph data structure linking obligations to code nodes with `Governs`, `Implements`, `Violates` | M3 | R3 Spec |
| F12 | CLI `needle audit` | CLI command generating compliance audit reports in console, markdown, and JSON formats | M3 | R3 Spec |
| F13 | MCP Compliance & Ledger Tools | Tools `get_obligations`, `check_compliance`, `get_compliance_report`, `verify_ledger`, `sign_ledger`, `get_ledger_status` registered in `src/cli/mcp/` | M3 | R3 Spec |
| F14 | Canonical JSON Block Encoding | Deterministic JSON serialization for block payloads and hashing | M4 | R4 Spec |
| F15 | SHA-256 Block & Payload Hashing | Cryptographic hash chaining using `sha2` (payload hash, signing preimage, block hash) | M4 | R4 Spec |
| F16 | Ed25519 Digital Signatures | Non-repudiable signing and verification via `ed25519-dalek` | M4 | R4 Spec |
| F17 | Redacted Keypair Security | Keypair management in `src/ledger/keypair.rs` with custom Debug/Display masking private keys | M4 | R4 Spec / Security |
| F18 | Append-Only JSONL Writer | Append audit reports to `.needle/ledger/audit_chain.jsonl` with CLI `needle ledger append` | M4 | R4 Spec |
| F19 | Fresh/Empty Chain Clean Verification | CLI `needle ledger verify` cleanly verifies empty or non-existent ledger returning 0 blocks | M4 | R4 Spec / AC |
| F20 | Tamper Detection & Localization | Verifier catches corrupted payloads, broken sequence, modified hashes, outputting exact broken sequence number | M4 | R4 Spec / AC |
| F21 | E2E Opaque-Box Test Suite | Comprehensive 4-tier requirement-driven E2E test suite covering F1-F20 | M5 (Test Track) | E2E Testing Track |
| F22 | Adversarial Coverage Hardening | Tier 5 white-box stress testing and edge-case validation | M5 (Phase 2) | E2E Testing Track |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Sovereign Build Mode & Local LLM Routing | F1, F2, F3, F4, F5, F6: `Cargo.toml` features (`cloud` vs `sovereign`), `src/llm.rs` loopback/offline-strict, `src/cli/doctor.rs` | none | IN_PROGRESS |
| M2 | Policy Ingestion & Obligation Structuring | F7, F8, F9: `src/policy/parser.rs`, `src/policy/clause.rs`, `src/policy/structurer.rs`, scanned PDF guard, `needle policy ingest` | none | PLANNED |
| M3 | Compliance Graph, Audit CLI & MCP Tools | F10, F11, F12, F13: `src/policy/matcher.rs`, `src/policy/graph.rs`, `src/policy/report.rs`, `src/cli/audit.rs`, `src/cli/mcp/` tools | M1, M2 | PLANNED |
| M4 | Cryptographic Audit Ledger Subsystem | F14, F15, F16, F17, F18, F19, F20: `src/ledger/` (`block.rs`, `crypto.rs`, `keypair.rs`, `verifier.rs`, `mod.rs`), `needle ledger` CLI | none | PLANNED |
| M5 | Final Milestone: E2E Test Pass & Adversarial Hardening | F21, F22: Pass 100% E2E test suite (Tiers 1-4) + Tier 5 Adversarial Coverage Hardening | M1, M2, M3, M4, Test Track | PLANNED |

## Interface Contracts

### `src/policy/` ↔ `src/query/` & `src/graph/`
- `evaluate_compliance(document: &PolicyDocument, query_engine: &QueryEngine, code_graph: &CodeGraph) -> Result<PolicyComplianceGraph, crate::Error>`
- Types: `PolicyDocument`, `PolicyClause`, `PolicyObligation`, `ObligationType`, `Severity`, `ComplianceStatus`, `ComplianceLink`, `PolicyComplianceGraph`
- Errors: `Error::PolicyError(String)`, `Error::InvalidPath(String)`

### `src/ledger/` ↔ CLI & Audit
- `append_to_ledger(ledger_path: &Path, keypair: &LedgerKeypair, entry_type: EntryType, payload: serde_json::Value) -> Result<LedgerBlock, Error>`
- `verify_ledger_file(ledger_path: &Path) -> Result<VerificationSummary, Error>`
- `LedgerKeypair::generate() -> Self`, `LedgerKeypair::from_bytes(bytes: &[u8; 32]) -> Self`, `LedgerKeypair::public_key_hex(&self) -> String`
- Security: `fmt::Debug` on `LedgerKeypair` yields `"[REDACTED PRIVATE KEY]"`
- Types: `LedgerBlock`, `EntryType`, `VerificationSummary`
- Errors: `Error::LedgerError(String)`

### `src/llm.rs` ↔ Sovereign Mode & Doctor
- `LlmConfig { offline_strict: bool, ... }`
- `LlmClient::complete(&self, system: &str, user: &str) -> Result<String, String>`
- Local Ollama loopback URL validation: `127.0.0.1:11434` or `localhost:11434`
- `doctor::verify_sovereign_readiness(offline_strict: bool) -> Result<DoctorReport, Error>`

## Code Layout
```
d:\AEGIS_AST\
├── Cargo.toml                  # Gated dependencies: cloud vs sovereign, sha2, ed25519-dalek
├── src/
│   ├── main.rs                 # CLI subcommands: Doctor, Policy, Audit, Ledger, ...
│   ├── lib.rs                  # Module exports: pub mod policy; pub mod ledger; ...
│   ├── error.rs                # Error variants: PolicyError, LedgerError, OfflineStrictViolation
│   ├── llm.rs                  # Sovereign Ollama routing, --offline-strict validation
│   ├── cli/
│   │   ├── doctor.rs           # needle doctor --sovereign implementation
│   │   ├── policy.rs           # needle policy ingest / list implementation
│   │   ├── audit.rs            # needle audit implementation
│   │   ├── ledger.rs           # needle ledger append / verify / keygen implementation
│   │   └── mcp/mod.rs          # MCP tools: get_obligations, check_compliance, get_compliance_report
│   ├── policy/
│   │   ├── mod.rs              # Policy subsystem root
│   │   ├── parser.rs           # PDF/text ingestion & scanned PDF guard
│   │   ├── clause.rs           # Data models (PolicyDocument, PolicyClause, PolicyObligation)
│   │   ├── structurer.rs       # LLM & rule-based obligation structuring
│   │   ├── matcher.rs          # QueryEngine search & CodeGraph node matching
│   │   ├── graph.rs            # PolicyComplianceGraph data structure
│   │   └── report.rs           # Compliance reporting (Console, Markdown, JSON)
│   └── ledger/
│       ├── mod.rs              # Ledger subsystem root & append API
│       ├── block.rs            # LedgerBlock & canonical serialization
│       ├── crypto.rs           # SHA-256 & Ed25519 cryptographic primitives
│       ├── keypair.rs          # Keypair management & private key redaction
│       └── verifier.rs         # Block validation & exact tamper localization
└── tests/
    └── e2e_sentinel_tests.rs   # Comprehensive 4-Tier E2E test suite
```
