# Handoff Report: Milestone M2 — Policy Ingestion & Obligation Structuring (Explorer 1)

## 1. Observation

Direct observations from codebase inspection and requirements analysis:

### 1.1 Existing Dependencies & Build System (`Cargo.toml`)
- **PDF Extraction**: `pdf-extract = "0.7"` is already declared in `Cargo.toml` (line 82). It provides `pdf_extract::extract_text(path: impl AsRef<Path>) -> Result<String, OutputError>`.
- **Serialization**: `serde = { version = "1.0", features = ["derive"] }` (line 36) and `serde_json = "1.0"` (line 37) are active.
- **Error Handling**: `thiserror = "1.0"` and `anyhow = "1.0"` are present (lines 54-55).
- **Time & IDs**: `chrono = "0.4"` (line 88) is available for RFC-3339 timestamps.
- **Regex**: `regex` is not in `Cargo.toml`. Clause segmentation and keyword pattern matching can be implemented deterministically using standard Rust string methods (`lines()`, `trim()`, `starts_with()`, `split()`, `to_lowercase()`), avoiding unnecessary crate dependencies.

### 1.2 Existing Codebase Architecture
- **Error Types (`src/error.rs`)**:
  - `Error` is a custom enum implementing `Display` and `std::error::Error`.
  - Currently contains variants: `Io`, `InvalidPath`, `IndexNotFound`, `ChunkingError`, `EmbeddingError`, `IndexError`, `QueryError`, `ConfigError`, `SerializationError`, `Other`.
  - Needs: `PolicyError(String)` variant and corresponding `Display` match arm `Error::PolicyError(e) => write!(f, "Policy error: {}", e)`.
- **Module Declarations (`src/lib.rs`)**:
  - Exports modules `analysis`, `chunking`, `config`, `embedding`, `error`, `graph`, `indexing`, `llm`, `query`, `schema`, `storage`, `server`, `watcher`.
  - Needs: `pub mod policy;` module registration.
- **LLM Infrastructure (`src/llm.rs`)**:
  - `LlmClient` has `complete(&self, system: &str, user: &str) -> Result<String, String>` (async).
  - Routes to Anthropic, OpenAI, Groq, or local Ollama (`http://127.0.0.1:11434/api/chat`).
- **Knowledge Graph & Query Engine (`src/graph/mod.rs`, `src/query/mod.rs`)**:
  - `CodeGraph` defines `GraphNode` with `id`, `name`, `kind: NodeKind`, `file_path`, `line_start`, `line_end`.
  - `QueryEngine::search(&self, query: &str, limit: usize, lang_filter: Option<Language>) -> Result<(Vec<SearchResult>, QueryTiming)>`.
  - `PolicyObligation` will provide `semantic_query` and `target_keywords` consumed by `QueryEngine` in Milestone M3.
- **Storage Layer (`src/storage/mod.rs`)**:
  - Project root is resolved via nearest `.git` directory (`.needle/`).
  - Policies should be stored under `.needle/policy/` (or `<project_root>/.needle/policy/<policy_id>.json`).

---

## 2. Logic Chain

From these observations, we derive the following architectural decisions and interfaces for Milestone M2:

### 2.1 Component Structure for `src/policy/`
The policy subsystem for M2 comprises three primary modules and one root re-exporter:
1. `src/policy/mod.rs`: Module exports and public type re-exports.
2. `src/policy/clause.rs`: Domain data models (`PolicyDocument`, `PolicyClause`, `PolicyObligation`, `ObligationType`, `Severity`).
3. `src/policy/parser.rs`: File ingestion (`.pdf`, `.md`, `.txt`, `.policy`), scanned PDF guard, and clause chunking.
4. `src/policy/structurer.rs`: Obligation extraction using LLM prompt + deterministic heuristic fallback.

### 2.2 Detailed Interface Specifications

#### A. Data Models (`src/policy/clause.rs`)
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
    pub id: String,                    // e.g. "POL-AUTH-001"
    pub clause_id: String,             // e.g. "CLAUSE-1.1"
    pub title: String,
    pub description: String,
    pub obligation_type: ObligationType,
    pub severity: Severity,
    pub target_keywords: Vec<String>,  // Lexical terms for BM25 search
    pub semantic_query: String,        // Natural language query for HNSW kNN
    pub ast_target_kinds: Vec<String>, // e.g. ["function", "method", "endpoint"]
    pub rule_criteria: String,         // Compliance validation rule
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyClause {
    pub id: String,                    // e.g. "CLAUSE-1"
    pub clause_number: String,         // e.g. "1.1" or "Section 2"
    pub title: String,
    pub raw_text: String,
    pub obligations: Vec<PolicyObligation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDocument {
    pub id: String,                    // Unique policy identifier
    pub name: String,                  // Display title
    pub version: String,               // Policy version (e.g. "1.0.0")
    pub source_path: String,           // Ingested file path
    pub content_hash: String,          // SHA-256 / XXH64 content hash
    pub clauses: Vec<PolicyClause>,
    pub created_at: String,            // ISO-8601 / RFC-3339 timestamp
}
```

#### B. Parser & Ingest Guard (`src/policy/parser.rs`)
- **Scanned PDF Edge Case**:
  When extracting text via `pdf_extract::extract_text(path)`, check:
  ```rust
  let printable_chars = text.chars().filter(|c| !c.is_whitespace() && !c.is_control()).count();
  if printable_chars < 20 {
      return Err(crate::Error::PolicyError(format!(
          "Scanned or image-only PDF detected at '{}': contains no extractable text (found {} printable characters). OCR is required before ingesting.",
          path.display(),
          printable_chars
      )));
  }
  ```
- **Supported Formats**:
  - `.pdf`: via `pdf-extract`
  - `.md`, `.markdown`, `.txt`, `.policy`, `.rst`: via `std::fs::read_to_string(path)`
  - Unsupported extension: returns `Error::PolicyError("Unsupported policy file format '.xyz'. Supported: .pdf, .md, .txt, .policy")`.
  - Non-existent path: returns `Error::InvalidPath("Policy file not found: ...")`.

#### C. Obligation Structurer (`src/policy/structurer.rs`)
- **LLM-Based Extraction**:
  - Calls `LlmClient::complete()` with a structured system prompt requesting JSON output.
  - Sanitizes markdown code fences (````json ... ````).
- **Heuristic Rule-Based Fallback**:
  - Deterministic keyword and pattern analyzer.
  - Matches modal verbs:
    - `"must"`, `"shall"`, `"required"` -> `Severity::High` or `Critical`
    - `"must not"`, `"prohibited"`, `"forbidden"`, `"shall not"` -> `Severity::Critical`
    - `"should"`, `"recommended"` -> `Severity::Medium`
    - `"may"`, `"optional"` -> `Severity::Low`
  - Matches security domains:
    - `"auth"`, `"login"`, `"password"`, `"token"` -> `ObligationType::Authentication`
    - `"permission"`, `"role"`, `"rbac"`, `"access"` -> `ObligationType::Authorization`
    - `"encrypt"`, `"tls"`, `"aes"`, `"cipher"`, `"https"` -> `ObligationType::EncryptionInTransit` / `EncryptionAtRest`
    - `"log"`, `"audit"`, `"trace"`, `"event"` -> `ObligationType::LoggingAndAudit`
    - `"sanitize"`, `"validate"`, `"injection"`, `"xss"` -> `ObligationType::InputValidation`
    - `"secret"`, `"private key"`, `"api key"`, `"credential"` -> `ObligationType::SecretManagement`
  - Generates sensible `semantic_query` and `target_keywords` from clause context.

#### D. CLI Interface (`src/cli/policy.rs` and `src/main.rs`)
- `needle policy ingest <path> [--name <name>] [--version <version>] [--dry-run]`
  - Parses document, chunks into clauses, extracts obligations.
  - Persists JSON document under `.needle/policy/<policy_id>.json` (unless `--dry-run`).
  - Emits formatted terminal summary showing clause count, obligation count, and severity breakdown.
- `needle policy list`
  - Reads `.needle/policy/*.json`, lists all active policies, version, clause count, and obligation counts.

---

## 3. Caveats

1. **`pdf-extract` Limitations**: Pure text stream extraction only. Cannot extract text embedded as bitmap images inside scanned PDFs (handled by explicit <20 printable char error guard).
2. **LLM Connectivity**: In offline/sovereign environments without an active Ollama daemon, the system must transparently fall back to the heuristic rule-based structurer without raising a fatal error.
3. **Zero-Panic Rule**: All file reads, JSON deserializations, and string operations must use `?` error propagation and return `crate::Result`. No `.unwrap()` or `.expect()` calls on user input paths.

---

## 4. Conclusion

The architecture for Milestone M2 is fully defined:
- `src/error.rs`: Add `PolicyError(String)` variant.
- `src/lib.rs`: Add `pub mod policy;`.
- `src/policy/clause.rs`: Complete data structures (`PolicyDocument`, `PolicyClause`, `PolicyObligation`, `ObligationType`, `Severity`).
- `src/policy/parser.rs`: Multi-format parsing with scanned PDF guard (<20 chars).
- `src/policy/structurer.rs`: Hybrid LLM + heuristic rule-based obligation structurer.
- `src/cli/policy.rs`: CLI subcommands for `ingest` and `list`.

---

## 5. Verification Method

To independently verify the implementation:
1. **Scanned PDF Guard Unit/Integration Test**:
   - Create a 0-byte or minimal image-only PDF file.
   - Run `parse_policy_file(&dummy_pdf_path)`.
   - Verify it returns `Err(Error::PolicyError(...))` containing `"Scanned or image-only PDF detected"`.
2. **Text / Markdown Policy Ingestion Test**:
   - Parse sample Markdown policy with headers and normative keywords.
   - Verify `PolicyDocument` produces valid `PolicyClause` and structured `PolicyObligation` items.
3. **Heuristic Fallback Test**:
   - Run `ObligationStructurer::extract_heuristic()` on text containing `"All endpoints must require JWT authentication."`.
   - Verify extracted obligation has `ObligationType::Authentication`, `Severity::High` (or `Critical`), and populated `target_keywords`.
4. **CLI Command Test**:
   - Run `cargo test policy` or `needle policy ingest sample.md`.
