# Authoritative Specification & Mining Report: Policy & Ledger Subsystems (R3 & R4)

This report provides the exhaustive specification for Requirement R3 (**Policy-Code Compliance Graph**, `src/policy/`) and Requirement R4 (**Cryptographic Audit Ledger**, `src/ledger/`) for NEEDLE-SENTINEL.

---

## 1. Observation

Direct observations from codebase inspection and requirements analysis:

1. **Workspace & Existing Modules**:
   - `Cargo.toml` lines 1-105: Defines dependencies including `clap` (v4.4), `tree-sitter` (0.20), `serde` / `serde_json`, `tokio` (1.35), `pdf-extract` (0.7), `rand` (0.8), `chrono` (0.4), `anyhow` / `thiserror`.
   - `src/lib.rs` lines 6-18: Currently exports `analysis`, `chunking`, `config`, `embedding`, `error`, `graph`, `indexing`, `llm`, `query`, `schema`, `storage`, `server`, `watcher`. Needs new modules `pub mod policy;` and `pub mod ledger;`.
   - `src/error.rs` lines 5-17: `pub enum Error` defines variants `Io`, `InvalidPath`, `IndexNotFound`, `ChunkingError`, `EmbeddingError`, `IndexError`, `QueryError`, `ConfigError`, `SerializationError`, `Other`. Needs variants `PolicyError(String)` and `LedgerError(String)`.
   - `src/cli/init.rs` lines 83-88: Demonstrates existing PDF text extraction via `pdf_extract::extract_text(path).ok()?` which silently discards extraction failures and ignores empty/scanned PDFs.
   - `src/llm.rs` lines 26-82: `LlmClient` provides `complete(system: &str, user: &str) -> Result<String, String>` routing across Anthropic, OpenAI, Groq, and local Ollama (`http://127.0.0.1:11434/api/chat`).
   - `src/graph/mod.rs` lines 42-101: `CodeGraph`, `GraphNode` (`NodeKind::Function`, `NodeKind::Method`, `NodeKind::Endpoint`, `NodeKind::Struct`, etc.), `GraphEdge`, `GraphStats`.
   - `src/query/mod.rs` lines 11-133: `QueryEngine::search(query, limit, lang_filter)` performs hybrid BM25 + HNSW kNN + Reciprocal Rank Fusion (RRF).
   - `src/cli/mcp/mod.rs` lines 198-500: Tool registration and dispatch mechanism (`tool_definitions()`, `handle_request()`, `dispatch_tool()`).
   - `src/main.rs` lines 15-125: CLI subcommand definitions using Clap derive.

2. **Required Cryptographic Crates**:
   - `sha2 = "0.10"`: Cryptographic SHA-256 for block and report payload hashing.
   - `ed25519-dalek = { version = "2.1", features = ["rand_core"] }`: Ed25519 signing and verification.

3. **Core Mandates & Constraints**:
   - Zero `unwrap()`, `expect()`, or `panic!()` on user-input paths (policy PDFs, text documents, ledger files, source files).
   - Private key security: The ledger private key must **never** be logged, printed, or emitted, even at debug/trace level. Custom `Debug` / `Display` implementations must redact private keys (`"[REDACTED PRIVATE KEY]"`).
   - Scanned PDF edge case: A scanned-image PDF or PDF with no extractable text (<20 printable chars) must fail loudly with a clear descriptive error and must not silently index or create an empty document.
   - Fresh ledger edge case: An empty or non-existent ledger chain must verify cleanly (returning success, 0 blocks verified).
   - Tamper detection: Verifier must detect any modified payload, altered sequence number, corrupted block hash, or broken Ed25519 signature and report the **exact sequence number** where tampering occurred.
   - Hand-rolled constraint: Compliance graph linking logic and audit ledger chaining/verification must be hand-rolled; no external copy-pasted blockchain/compliance libraries.

---

## 2. Logic Chain

1. **Architecture of Policy Subsystem (`src/policy/`)**:
   - Ingestion starts with `parser.rs` reading `.pdf`, `.md`, `.txt`, or `.policy` files. It evaluates the extracted text length and character distribution. If the document is a scanned image with no extractable text, it returns an explicit error (`Error::PolicyError`).
   - `clause.rs` defines `PolicyDocument`, `PolicyClause`, `PolicyObligation`, `ObligationType`, and `Severity`.
   - `structurer.rs` accepts raw clause text and uses `LlmClient` (configured for local Ollama in sovereign mode) to parse clauses into structured obligations with semantic queries, target keywords, and rule descriptions. A resilient fallback heuristic parser extracts obligations based on standard normative terms ("MUST", "SHALL", "REQUIRED") if the LLM is unavailable.
   - `matcher.rs` takes each obligation and executes hybrid queries via `QueryEngine::search()` against the indexed codebase. It cross-references chunk hits with AST nodes in `CodeGraph` (`GraphNode`) and traces caller/callee paths.
   - `graph.rs` constructs the `PolicyComplianceGraph`, storing nodes (Obligations, Code Nodes) and edges (`Governs`, `Implements`, `Violates`, `Unmapped`).
   - `report.rs` computes compliance statistics, builds `ComplianceReport`, and serializes findings into Console, Markdown, and canonical JSON.

2. **Architecture of Cryptographic Audit Ledger Subsystem (`src/ledger/`)**:
   - `block.rs` defines `LedgerBlock`, `EntryType`, and canonical JSON serialization for deterministic hashing.
   - `crypto.rs` implements SHA-256 block hashing and payload hashing, plus Ed25519 digital signature creation and verification.
   - Hashing sequence is strictly ordered:
     1. `payload_hash = SHA-256(canonical_json(block.payload))`
     2. `signing_preimage = "{sequence}:{timestamp}:{prev_hash}:{entry_type}:{payload_hash}"`
     3. `signature = ed25519_sign(private_key, signing_preimage)`
     4. `block_hash = SHA-256("{sequence}:{timestamp}:{prev_hash}:{entry_type}:{payload_hash}:{signer_public_key}:{signature}")`
   - `keypair.rs` manages Ed25519 keypair generation using `rand::rngs::OsRng` and storage at `.needle/ledger/key.priv` and `.needle/ledger/key.pub`. It implements a redacted `fmt::Debug` ensuring private keys are never leaked.
   - `verifier.rs` reads `.needle/ledger/audit_chain.jsonl` sequentially. If empty, it returns `Ok(0 blocks)`. For each block, it validates:
     - `sequence == prev_sequence + 1` (or `0` for genesis)
     - `prev_hash == prev_block.block_hash` (or 64 zeroes for genesis)
     - `payload_hash == SHA-256(canonical_json(block.payload))`
     - `ed25519_verify(signer_public_key, signing_preimage, signature) == true`
     - `block_hash == SHA-256(block_preimage)`
     If any check fails, it returns `Err` with the exact sequence number: `"TAMPER DETECTED at sequence {sequence}: {details}"`.

---

## 3. Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | Policy / Ingest | PDF Policy Parser | Extracts text from PDF policies via `pdf-extract` with character validation | File path (`&Path`) | `ExtractedDocument { title, text, metadata }` | Returns `Error::PolicyError` on missing file, corrupt PDF, or scanned PDF without text | R3 Spec / `src/policy/parser.rs` |
| 2 | Policy / Ingest | Scanned PDF Guard | Fails loudly if a PDF contains only scanned images or <20 printable characters | PDF text buffer | Validated text string | Returns `Error::PolicyError("Scanned or image-only PDF detected...")` | R3 Spec / Acceptance Criteria |
| 3 | Policy / Ingest | Plaintext / Markdown Policy Parser | Parses `.txt`, `.md`, `.policy`, `.rst` policy files with section boundary extraction | File path (`&Path`) | `ExtractedDocument` | Returns `Error::Io` on read error, `Error::PolicyError` if empty | R3 Spec / `src/policy/parser.rs` |
| 4 | Policy / Structuring | Clause Segmentation | Segments raw policy text into clauses using regex patterns for Section/Article/Markdown headers | Document text | `Vec<PolicyClause>` | Returns empty/single clause fallback without crashing | R3 Spec / `src/policy/clause.rs` |
| 5 | Policy / Structuring | LLM Obligation Structuring | Uses `LlmClient` to convert raw clauses into structured `PolicyObligation` with queries and keywords | Raw clause text, `LlmClient` | `Vec<PolicyObligation>` | Falls back to rule-based keyword extraction if LLM fails or offline | R3 Spec / `src/policy/structurer.rs` |
| 6 | Policy / Structuring | Heuristic Rule-Based Fallback | Extracts obligations when running in offline-strict mode without active Ollama | Raw clause text | `Vec<PolicyObligation>` | Deterministic fallback; returns empty list only if text has no obligation patterns | R3 Spec / `src/policy/structurer.rs` |
| 7 | Policy / Matching | Hybrid Code Matching | Queries `QueryEngine` using semantic query + target keywords from obligation | `PolicyObligation`, `QueryEngine` | `Vec<SearchResult>` | Returns `QueryError` if query engine fails | R3 Spec / `src/policy/matcher.rs` |
| 8 | Policy / Matching | AST Node Resolution | Resolves search hits to `CodeGraph` AST symbols (`GraphNode`) and traces call paths | `Vec<SearchResult>`, `CodeGraph` | `Vec<ComplianceLink>` | Gracefully skips unindexed nodes; handles empty graph | R3 Spec / `src/policy/matcher.rs` |
| 9 | Policy / Matching | Compliance Evaluation | Evaluates status (`Compliant`, `NonCompliant`, `PartiallyCompliant`, `Unmapped`) | `PolicyObligation`, `Vec<ComplianceLink>` | `EvaluatedObligation` | Marks `Unmapped` if no code matches; `ManualReviewRequired` for low confidence | R3 Spec / `src/policy/matcher.rs` |
| 10 | Policy / Graph | Compliance Graph Construction | Builds graph linking policy obligations to AST nodes with `Governs`, `Implements`, `Violates` edges | Evaluated obligations, `CodeGraph` | `PolicyComplianceGraph` | Returns empty graph if no policies ingested | R3 Spec / `src/policy/graph.rs` |
| 11 | Policy / Reporting | Compliance Report Generator | Generates summary metrics, score percentage, and detailed audit findings | `PolicyComplianceGraph`, policy metadata | `ComplianceReport` | Serializes cleanly to Console, Markdown, and JSON | R3 Spec / `src/policy/report.rs` |
| 12 | Policy / CLI | `needle policy ingest` | CLI command to ingest and parse a policy document, saving obligations | `--name <name>`, `--version <ver>`, `path` | Prints clause/obligation count; saves to `.needle/policy/` | Fails with non-zero exit code and clear error message on invalid/scanned input | R3 Spec / `src/cli/policy.rs` |
| 13 | Policy / CLI | `needle audit` | CLI command to execute full codebase compliance audit against active policies | `--policy <id>`, `--format <fmt>`, `--output <path>`, `--fail-on-violation` | Structured audit report; optional ledger block creation | Exits with code 1 if `--fail-on-violation` and violations found | R3 Spec / `src/cli/audit.rs` |
| 14 | Policy / MCP | `get_obligations` | MCP tool to query extracted obligations with optional filtering | `policy_id`, `obligation_type`, `severity` | JSON array of obligations | Returns JSON-RPC error if policy index is missing | R3 Spec / MCP tools |
| 15 | Policy / MCP | `check_compliance` | MCP tool to inspect compliance status of a specific obligation or source file | `obligation_id`, `file_path` | JSON compliance details with evidence snippets | Returns JSON-RPC error if obligation or file not found | R3 Spec / MCP tools |
| 16 | Policy / MCP | `get_compliance_report` | MCP tool to generate complete compliance report across all policies | `policy_id`, `format`, `min_severity` | JSON formatted report and summary statistics | Returns JSON-RPC error if audit fails | R3 Spec / MCP tools |
| 17 | Ledger / Block | Canonical JSON Serialization | Serializes payload data deterministically for reproducible hashing | `serde_json::Value` / Struct | Canonical byte buffer | Returns `Error::SerializationError` on un-serializable value | R4 Spec / `src/ledger/block.rs` |
| 18 | Ledger / Crypto | SHA-256 Hashing | Computes 256-bit hash for payload and block header | Byte slice `&[u8]` | 64-char lowercase hex string | Infallible | R4 Spec / `src/ledger/crypto.rs` |
| 19 | Ledger / Crypto | Ed25519 Signing & Verification | Signs signing preimage with private key and verifies with public key | Preimage string, `SigningKey` / `VerifyingKey` | 128-char hex signature / boolean result | Returns `LedgerError::SignatureError` on malformed key or signature | R4 Spec / `src/ledger/crypto.rs` |
| 20 | Ledger / Keys | Key Management & Generation | Generates and loads Ed25519 keypair from `.needle/ledger/key.priv` and `.needle/ledger/key.pub` | Key file path | `LedgerKeypair` | Returns `LedgerError::KeyNotFound` or `Io` error on failure | R4 Spec / `src/ledger/keypair.rs` |
| 21 | Ledger / Security | Private Key Redaction | Custom `Debug` and `Display` implementation masking private key bytes | `LedgerKeypair` | `"[REDACTED PRIVATE KEY]"` | Prevents leaking secret key material in logs/panics/errors | R4 Spec / Security Constraint |
| 22 | Ledger / Block | Block Hashing Chaining | Computes `block_hash` over sequence, timestamp, prev_hash, entry_type, payload_hash, pubkey, signature | Block fields | 64-char hex block hash | Infallible | R4 Spec / `src/ledger/block.rs` |
| 23 | Ledger / Storage | Append-Only JSONL Writer | Appends serialized block as single JSON line to `.needle/ledger/audit_chain.jsonl` | `LedgerBlock` | Append status, new sequence number | Returns `Error::Io` on disk write failure | R4 Spec / `src/ledger/mod.rs` |
| 24 | Ledger / Verifier | Clean Empty Chain Verification | Verifies non-existent or 0-byte ledger file without error | Path to ledger file | `VerificationResult { total_blocks: 0, valid: true }` | Returns `Ok`, never errors on fresh chain | R4 Spec / Acceptance Criteria |
| 25 | Ledger / Verifier | Sequential Integrity Check | Validates `sequence` numbers are consecutive starting from 0 | Ledger stream | Sequence validation status | Halts and returns `TamperDetected` at exact sequence number on gap/reset | R4 Spec / `src/ledger/verifier.rs` |
| 26 | Ledger / Verifier | Chain Hash & Payload Verification | Recomputes payload hash, block hash, and verifies Ed25519 signature per block | Ledger stream | Verification status per block | Halts and returns `TamperDetected` with exact sequence number and failure reason | R4 Spec / `src/ledger/verifier.rs` |
| 27 | Ledger / CLI | `needle ledger append` | CLI command to append an audit report or payload to the ledger | `--report <path>`, `--type <type>`, `--key <path>` | Prints sequence, block hash, timestamp, signer | Exits with error if report file missing or invalid | R4 Spec / `src/cli/ledger.rs` |
| 28 | Ledger / CLI | `needle ledger verify` | CLI command to verify ledger integrity and detect tampering | `--ledger <path>`, `--public-key <path>`, `--verbose` | Prints verification summary or exact sequence number of tampering | Exits with code 0 on success, code 1 on tamper detection | R4 Spec / `src/cli/ledger.rs` |

---

## 4. Edge Cases

| # | Feature | Input | Observed / Required Behavior |
|---|---------|-------|-----------------------------|
| 1 | `pdf-extract` Ingestion | Scanned image PDF with 0 extractable text characters | **Fails loudly** with `Error::PolicyError("Scanned or image-only PDF detected at '...': contains no extractable text. OCR is required.")`. Does NOT create an empty document. |
| 2 | `pdf-extract` Ingestion | Whitespace-only or near-empty PDF (<20 printable chars) | Fails loudly with `Error::PolicyError` indicating insufficient text content. |
| 3 | Policy Ingestion | Non-existent policy file path (`needle policy ingest foo.pdf`) | Returns `Error::InvalidPath` or `Error::Io` with clear user-facing error message. No panic/unwrap. |
| 4 | Policy Ingestion | Invalid / corrupted PDF binary | Returns `Error::PolicyError` containing parser failure details. No panic/unwrap. |
| 5 | Policy Ingestion | Large PDF (e.g. 500+ pages) | Streams pages/paragraphs safely without memory exhaustion; chunks clauses within max token windows. |
| 6 | Clause Structuring | LLM returns markdown code fences (````json ... ````) or conversational chatter | Structurer strips code fences and extracts the embedded JSON payload cleanly. |
| 7 | Clause Structuring | LLM offline / unavailable / non-responsive | Falls back gracefully to heuristic rule-based extractor scanning for normative keywords ("MUST", "SHALL", "REQUIRED"). |
| 8 | Code Matching | Policy obligation has no matching code in codebase | Correctly identifies obligation as `ComplianceStatus::Unmapped`; does not crash or omit from report. |
| 9 | Code Matching | Codebase not indexed (`.needle/index` missing) | Returns `Error::IndexNotFound("Index not found. Run `needle init <dirs...>` first.")`. |
| 10 | Ledger Verification | Non-existent `.needle/ledger/audit_chain.jsonl` (fresh project) | Verifies cleanly: returns `Ok(0 blocks verified)` and exit code 0. |
| 11 | Ledger Verification | 0-byte or whitespace-only ledger file | Verifies cleanly: returns `Ok(0 blocks verified)` and exit code 0. |
| 12 | Ledger Tampering | Single character modified in `payload` of block 3 | `needle ledger verify` fails immediately with `"TAMPER DETECTED at sequence 3: payload_hash mismatch (expected ..., computed ...)"` and exits with code 1. |
| 13 | Ledger Tampering | Block sequence number altered (e.g. 0 -> 2, skipping 1) | `needle ledger verify` fails immediately with `"TAMPER DETECTED at sequence 2: sequence discontinuity (expected 1, found 2)"` and exits with code 1. |
| 14 | Ledger Tampering | Block deleted or removed from middle of chain | `needle ledger verify` fails immediately with `"TAMPER DETECTED at sequence {N}: prev_hash mismatch"` and exits with code 1. |
| 15 | Ledger Tampering | Signature corrupted or re-signed with unauthorized key | `needle ledger verify` fails immediately with `"TAMPER DETECTED at sequence {N}: invalid Ed25519 signature"` and exits with code 1. |
| 16 | Ledger Key Management | `tracing::debug!("{:?}", keypair)` executed | Output masks signing key: `LedgerKeypair { verifying_key: "...", signing_key: "[REDACTED PRIVATE KEY]" }`. Zero key leakage. |
| 17 | Ledger Key Management | Missing private key during `needle ledger append` | Prompts or generates key if `--gen-key-if-missing` set; otherwise returns `LedgerError::KeyNotFound` with instruction to generate key. |

---

## 5. Detailed Component Specifications

### 5.1 Policy Subsystem (`src/policy/`)

#### Types & Data Structures (`src/policy/clause.rs`)
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationType {
    Authentication,
    Authorization,
    EncryptionAtRest,
    EncryptionInTransit,
    DataRetention,
    DataSanitization,
    LoggingAndAudit,
    ErrorHandling,
    NetworkIsolation,
    InputValidation,
    SecretManagement,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Informational,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyObligation {
    pub id: String,                    // e.g. "POL-SEC-001"
    pub clause_id: String,             // e.g. "CLAUSE-4.2"
    pub title: String,
    pub description: String,
    pub obligation_type: ObligationType,
    pub severity: Severity,
    pub target_keywords: Vec<String>,  // Lexical keywords for BM25
    pub semantic_query: String,        // Natural language query for HNSW
    pub ast_target_kinds: Vec<String>, // e.g. ["function", "method", "endpoint"]
    pub rule_criteria: String,         // Compliance evaluation rule
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyClause {
    pub id: String,
    pub clause_number: String,
    pub title: String,
    pub raw_text: String,
    pub obligations: Vec<PolicyObligation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDocument {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source_path: String,
    pub content_hash: String,
    pub clauses: Vec<PolicyClause>,
    pub created_at: String,
}
```

#### Policy Text & PDF Extractor (`src/policy/parser.rs`)
```rust
use crate::{Error, Result};
use std::path::Path;

pub struct ExtractedDocument {
    pub title: String,
    pub text: String,
    pub source_path: String,
}

pub fn parse_policy_file(path: &Path) -> Result<ExtractedDocument> {
    if !path.exists() {
        return Err(Error::InvalidPath(format!("Policy file not found: {}", path.display())));
    }

    let ext = path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let text = match ext.as_str() {
        "pdf" => {
            let extracted = pdf_extract::extract_text(path)
                .map_err(|e| Error::PolicyError(format!("Failed to parse PDF '{}': {e}", path.display())))?;
            
            // Check for scanned / image-only PDF edge case (<20 non-whitespace printable chars)
            let printable_chars = extracted.chars().filter(|c| !c.is_whitespace() && !c.is_control()).count();
            if printable_chars < 20 {
                return Err(Error::PolicyError(format!(
                    "Scanned or image-only PDF detected at '{}': contains no extractable text (found {} printable characters). OCR is required before ingesting.",
                    path.display(),
                    printable_chars
                )));
            }
            extracted
        }
        "txt" | "md" | "markdown" | "policy" | "rst" => {
            std::fs::read_to_string(path)
                .map_err(|e| Error::Io(e))?
        }
        other => {
            return Err(Error::PolicyError(format!(
                "Unsupported policy file format '.{}'. Supported formats: .pdf, .md, .txt, .policy",
                other
            )));
        }
    };

    let title = path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled Policy".to_string());

    Ok(ExtractedDocument {
        title,
        text,
        source_path: path.to_string_lossy().to_string(),
    })
}
```

#### Matcher & Compliance Engine (`src/policy/matcher.rs` & `src/policy/graph.rs`)
```rust
use crate::graph::CodeGraph;
use crate::query::QueryEngine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceStatus {
    Compliant,
    NonCompliant,
    PartiallyCompliant,
    Unmapped,
    ManualReviewRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceLink {
    pub obligation_id: String,
    pub node_id: Option<u32>,
    pub file_path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub symbol_name: Option<String>,
    pub status: ComplianceStatus,
    pub confidence: f32,
    pub evidence: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyComplianceGraph {
    pub policy_id: String,
    pub policy_name: String,
    pub links: Vec<ComplianceLink>,
    pub timestamp: String,
}

pub fn evaluate_compliance(
    document: &PolicyDocument,
    query_engine: &QueryEngine,
    code_graph: &CodeGraph,
) -> Result<PolicyComplianceGraph, crate::Error> {
    let mut links = Vec::new();

    for clause in &document.clauses {
        for obligation in &clause.obligations {
            let (results, _) = query_engine.search(&obligation.semantic_query, 10, None)?;
            
            if results.is_empty() {
                links.push(ComplianceLink {
                    obligation_id: obligation.id.clone(),
                    node_id: None,
                    file_path: String::new(),
                    line_start: 0,
                    line_end: 0,
                    symbol_name: None,
                    status: ComplianceStatus::Unmapped,
                    confidence: 1.0,
                    evidence: "No matching code implementation found in indexed codebase.".to_string(),
                    remediation: Some(format!("Implement code satisfying obligation: {}", obligation.title)),
                });
                continue;
            }

            for result in results.iter().take(3) {
                // Cross-reference with CodeGraph nodes
                let matched_node = code_graph.nodes.iter().find(|n| {
                    n.file_path == result.file_path &&
                    n.line_start <= result.line_end &&
                    n.line_end >= result.line_start
                });

                let symbol_name = matched_node.map(|n| n.name.clone());
                let node_id = matched_node.map(|n| n.id);

                links.push(ComplianceLink {
                    obligation_id: obligation.id.clone(),
                    node_id,
                    file_path: result.file_path.clone(),
                    line_start: result.line_start,
                    line_end: result.line_end,
                    symbol_name,
                    status: ComplianceStatus::Compliant,
                    confidence: result.score.min(1.0),
                    evidence: result.content.clone(),
                    remediation: None,
                });
            }
        }
    }

    Ok(PolicyComplianceGraph {
        policy_id: document.id.clone(),
        policy_name: document.name.clone(),
        links,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}
```

---

### 5.2 Cryptographic Audit Ledger Subsystem (`src/ledger/`)

#### Block Structure & Canonical Encoding (`src/ledger/block.rs`)
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    ComplianceAudit,
    SecurityScan,
    PolicyIngest,
    CodebaseSnapshot,
    SystemEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerBlock {
    pub sequence: u64,
    pub timestamp: String,
    pub prev_hash: String,
    pub entry_type: EntryType,
    pub payload_hash: String,
    pub payload: serde_json::Value,
    pub signer_public_key: String,
    pub signature: String,
    pub block_hash: String,
}

impl LedgerBlock {
    pub fn signing_preimage(
        sequence: u64,
        timestamp: &str,
        prev_hash: &str,
        entry_type: &EntryType,
        payload_hash: &str,
    ) -> String {
        format!("{}:{}:{}:{:?}:{}", sequence, timestamp, prev_hash, entry_type, payload_hash)
    }

    pub fn block_preimage(
        sequence: u64,
        timestamp: &str,
        prev_hash: &str,
        entry_type: &EntryType,
        payload_hash: &str,
        signer_public_key: &str,
        signature: &str,
    ) -> String {
        format!(
            "{}:{}:{}:{:?}:{}:{}:{}",
            sequence, timestamp, prev_hash, entry_type, payload_hash, signer_public_key, signature
        )
    }
}
```

#### Cryptographic Keypair & Redaction (`src/ledger/keypair.rs`)
```rust
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use std::fmt;

pub struct LedgerKeypair {
    pub(crate) signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl LedgerKeypair {
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        Self { signing_key, verifying_key }
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(bytes);
        let verifying_key = signing_key.verifying_key();
        Self { signing_key, verifying_key }
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key.to_bytes())
    }
}

// STRICT SECURITY RULE: Private key must NEVER be printed in Debug or Display
impl fmt::Debug for LedgerKeypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LedgerKeypair")
            .field("verifying_key", &hex::encode(self.verifying_key.to_bytes()))
            .field("signing_key", &"[REDACTED PRIVATE KEY]")
            .finish()
    }
}

impl fmt::Display for LedgerKeypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LedgerKeypair(pubkey: {})", hex::encode(self.verifying_key.to_bytes()))
    }
}
```

#### Verification & Tamper Localization (`src/ledger/verifier.rs`)
```rust
use crate::ledger::block::{EntryType, LedgerBlock};
use crate::ledger::crypto::{sha256_hex, verify_ed25519_signature};
use crate::Result;
use std::path::Path;

pub const GENESIS_PREV_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug)]
pub struct VerificationSummary {
    pub total_blocks: u64,
    pub is_valid: bool,
    pub latest_block_hash: Option<String>,
}

pub fn verify_ledger_file(ledger_path: &Path) -> Result<VerificationSummary> {
    if !ledger_path.exists() {
        // Edge Case R4: Empty / non-existent chain verifies cleanly
        return Ok(VerificationSummary {
            total_blocks: 0,
            is_valid: true,
            latest_block_hash: None,
        });
    }

    let file_content = std::fs::read_to_string(ledger_path)?;
    let lines: Vec<&str> = file_content.lines().filter(|l| !l.trim().is_empty()).collect();

    if lines.is_empty() {
        return Ok(VerificationSummary {
            total_blocks: 0,
            is_valid: true,
            latest_block_hash: None,
        });
    }

    let mut prev_hash = GENESIS_PREV_HASH.to_string();
    let mut expected_sequence = 0u64;

    for (line_num, line) in lines.iter().enumerate() {
        let block: LedgerBlock = serde_json::from_str(line).map_err(|e| {
            crate::Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: invalid JSON block structure: {e}",
                line_num
            ))
        })?;

        // 1. Validate sequence number continuity
        if block.sequence != expected_sequence {
            return Err(crate::Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: sequence discontinuity (expected {}, found {})",
                block.sequence, expected_sequence, block.sequence
            )));
        }

        // 2. Validate previous block hash chaining
        if block.prev_hash != prev_hash {
            return Err(crate::Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: prev_hash mismatch (expected {}, found {})",
                block.sequence, prev_hash, block.prev_hash
            )));
        }

        // 3. Validate payload hash
        let canonical_payload = serde_json::to_vec(&block.payload)?;
        let computed_payload_hash = sha256_hex(&canonical_payload);
        if block.payload_hash != computed_payload_hash {
            return Err(crate::Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: payload_hash mismatch (expected {}, computed {})",
                block.sequence, block.payload_hash, computed_payload_hash
            )));
        }

        // 4. Validate Ed25519 signature
        let signing_preimage = LedgerBlock::signing_preimage(
            block.sequence,
            &block.timestamp,
            &block.prev_hash,
            &block.entry_type,
            &block.payload_hash,
        );

        let sig_valid = verify_ed25519_signature(
            &block.signer_public_key,
            signing_preimage.as_bytes(),
            &block.signature,
        ).unwrap_or(false);

        if !sig_valid {
            return Err(crate::Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: invalid Ed25519 signature",
                block.sequence
            )));
        }

        // 5. Validate block hash
        let block_preimage = LedgerBlock::block_preimage(
            block.sequence,
            &block.timestamp,
            &block.prev_hash,
            &block.entry_type,
            &block.payload_hash,
            &block.signer_public_key,
            &block.signature,
        );
        let computed_block_hash = sha256_hex(block_preimage.as_bytes());
        if block.block_hash != computed_block_hash {
            return Err(crate::Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: block_hash mismatch (expected {}, computed {})",
                block.sequence, block.block_hash, computed_block_hash
            )));
        }

        prev_hash = block.block_hash.clone();
        expected_sequence += 1;
    }

    Ok(VerificationSummary {
        total_blocks: lines.len() as u64,
        is_valid: true,
        latest_block_hash: Some(prev_hash),
    })
}
```

---

### 5.3 CLI Interface Specification

#### Subcommands to add in `src/main.rs`:
```rust
#[derive(clap::Subcommand)]
enum Commands {
    // ... existing commands (Init, Search, Status, Reindex, Config, Bench, Watch, Mcp, Serve, Report, Graph) ...

    /// Policy ingestion and management
    Policy {
        #[command(subcommand)]
        action: PolicyCommands,
    },

    /// Run a compliance audit against ingested policies and code AST
    Audit {
        /// Optional policy ID to audit against (defaults to all active policies)
        #[arg(short, long)]
        policy: Option<String>,

        /// Output format: console, markdown, json
        #[arg(short, long, default_value = "console")]
        format: String,

        /// Output file path for audit report
        #[arg(short, long)]
        output: Option<String>,

        /// Minimum severity threshold: informational, low, medium, high, critical
        #[arg(short, long, default_value = "low")]
        severity: String,

        /// Exit with non-zero status if any compliance violations are detected
        #[arg(long)]
        fail_on_violation: bool,

        /// Automatically record and cryptographically sign this audit in the ledger
        #[arg(long)]
        sign_ledger: bool,
    },

    /// Cryptographic audit ledger commands
    Ledger {
        #[command(subcommand)]
        action: LedgerCommands,
    },
}

#[derive(clap::Subcommand)]
pub enum PolicyCommands {
    /// Ingest a PDF, Markdown, or text policy document
    Ingest {
        /// Path to policy document (.pdf, .md, .txt)
        #[arg(required = true)]
        path: String,

        /// Custom policy name/identifier
        #[arg(short, long)]
        name: Option<String>,

        /// Policy version
        #[arg(short, long, default_value = "1.0.0")]
        version: String,

        /// Dry-run mode: parse and extract without saving to index
        #[arg(long)]
        dry_run: bool,
    },

    /// List all ingested policy documents and obligations
    List,
}

#[derive(clap::Subcommand)]
pub enum LedgerCommands {
    /// Append an audit report or payload to the cryptographic ledger
    Append {
        /// Path to JSON report file
        #[arg(short, long, required = true)]
        report: String,

        /// Entry type: compliance_audit, security_scan, policy_ingest, codebase_snapshot
        #[arg(short, long, default_value = "compliance_audit")]
        r#type: String,

        /// Path to Ed25519 private key
        #[arg(short, long)]
        key: Option<String>,

        /// Automatically generate Ed25519 keypair if missing
        #[arg(long)]
        gen_key_if_missing: bool,
    },

    /// Verify the cryptographic integrity of the ledger chain
    Verify {
        /// Custom path to ledger file (default: .needle/ledger/audit_chain.jsonl)
        #[arg(short, long)]
        ledger: Option<String>,

        /// Show verification status of each individual block
        #[arg(short, long)]
        verbose: bool,
    },

    /// Generate a new Ed25519 keypair for ledger signing
    Keygen {
        /// Output directory for keypair
        #[arg(short, long)]
        output_dir: Option<String>,
        
        /// Overwrite existing keypair
        #[arg(long)]
        force: bool,
    },
}
```

---

### 5.4 MCP Tool Schemas

To be registered in `src/cli/mcp/mod.rs`:

```json
[
  {
    "name": "get_obligations",
    "description": "Retrieve extracted compliance obligations from ingested policies, with optional filters by policy ID, category, or severity.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "policy_id": { "type": "string", "description": "Optional policy ID to filter obligations" },
        "obligation_type": { "type": "string", "description": "Filter by type: authentication, authorization, encryption_at_rest, encryption_in_transit, data_retention, logging_and_audit, etc." },
        "severity": { "type": "string", "description": "Filter by minimum severity: informational, low, medium, high, critical" },
        "limit": { "type": "integer", "description": "Max obligations to return (default 20)", "default": 20 }
      }
    }
  },
  {
    "name": "check_compliance",
    "description": "Check compliance of a specific obligation or inspect compliance status for a specific source file against active policies.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "obligation_id": { "type": "string", "description": "Specific obligation ID to check (e.g. POL-SEC-001)" },
        "file_path": { "type": "string", "description": "Source file path to check compliance against policies" }
      }
    }
  },
  {
    "name": "get_compliance_report",
    "description": "Generate an end-to-end policy compliance audit report for the codebase, returning score percentage, mapped code nodes, and any policy violations.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "policy_id": { "type": "string", "description": "Optional policy ID to audit" },
        "format": { "type": "string", "enum": ["summary", "detailed", "json"], "default": "summary" },
        "min_severity": { "type": "string", "description": "Minimum severity to include in report (default low)" }
      }
    }
  }
]
```

---

## 6. Caveats

1. **OCR / Scanned PDFs**:
   `pdf-extract` is a text-stream parser and does not include an embedded Tesseract/OCR engine. When ingesting scanned image PDFs, the parser will detect 0 extractable characters and fail loudly with `Error::PolicyError`. This is per specification and prevents silent false positives.
2. **Local LLM Availability**:
   When running in sovereign mode, `LlmClient` requires a local Ollama instance running at `127.0.0.1:11434`. If Ollama is not running, the policy structurer falls back cleanly to heuristic rule-based keyword extraction without throwing a hard error.
3. **Ledger Concurrency**:
   Appending to `audit_chain.jsonl` should utilize file locks (`fs2` or standard lockfile) when multiple processes or CLI instances run concurrently to prevent sequence race conditions.
4. **Key Security**:
   Key files on disk should be created with restricted permissions (0600 on Unix systems).

---

## 7. Conclusion

Requirements R3 and R4 establish an air-gapped, verifiable security governance platform within Needle:
- **R3 Policy-Code Compliance Graph**: Robust PDF and text parsing, loud failure on scanned PDFs, LLM-powered and heuristic obligation structuring, hybrid search integration with `QueryEngine` and AST `CodeGraph`, and full CLI + MCP coverage.
- **R4 Cryptographic Audit Ledger**: Strict sequential hash-chaining (SHA-256), non-repudiable Ed25519 digital signatures, redacted key management preventing key leakage, clean zero-block verification on fresh chains, and tamper localization providing the exact sequence number upon corruption.

---

## 8. Verification Method

To independently verify the implementation during and after development:

1. **Scanned PDF Edge Case Verification**:
   - Create a dummy scanned PDF file with no text stream (or empty text).
   - Execute: `needle policy ingest scanned_test.pdf`
   - **Expected Result**: Fails loudly with exit code 1, emitting `"Scanned or image-only PDF detected... contains no extractable text."`

2. **Policy Ingestion & Audit Verification**:
   - Create a sample policy document `security_policy.md` containing password hashing and encryption obligations.
   - Execute: `needle policy ingest security_policy.md --name "SecPolicy"`
   - Execute: `needle audit --format json`
   - **Expected Result**: Ingest succeeds, reporting extracted clauses. Audit produces compliance graph links connecting obligations to matching functions/endpoints in indexed code.

3. **Fresh Ledger Clean Verification**:
   - Ensure `.needle/ledger/audit_chain.jsonl` does not exist.
   - Execute: `needle ledger verify`
   - **Expected Result**: Returns cleanly with exit code 0 and message `"Ledger verified: 0 blocks (empty chain)."`.

4. **Append & Tamper Detection Verification**:
   - Execute: `needle audit --output audit_report.json`
   - Execute: `needle ledger append --report audit_report.json --gen-key-if-missing`
   - Execute: `needle ledger verify` -> **Expected Result**: `"Ledger verified: 1 blocks valid. Chain integrity intact."` (exit code 0).
   - Modify one character inside `audit_chain.jsonl` on block 0 (e.g. edit a letter in payload).
   - Execute: `needle ledger verify`
   - **Expected Result**: Exits with code 1 and outputs `"TAMPER DETECTED at sequence 0: payload_hash mismatch..."`.

5. **Private Key Redaction Verification**:
   - Write a unit test:
     ```rust
     let kp = LedgerKeypair::generate();
     let debug_str = format!("{:?}", kp);
     assert!(!debug_str.contains(&hex::encode(kp.signing_key.to_bytes())));
     assert!(debug_str.contains("[REDACTED PRIVATE KEY]"));
     ```
   - Execute: `cargo test policy ledger`
