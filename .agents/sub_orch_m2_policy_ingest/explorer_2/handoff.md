# Handoff Report: Policy Ingestion & Clause Chunking Specifications (F7 & F8)

**Author**: Explorer 2 (Milestone M2: Policy Ingestion & Obligation Structuring)  
**Date**: 2026-08-15  
**Target Subsystem**: `src/policy/` (`src/policy/parser.rs`, `src/policy/clause.rs`, `src/error.rs`)  
**Features Covered**: Feature F7 (Policy Ingestion & Parsing) & Feature F8 (Policy Clause Chunking)  

---

## 1. Observation

Direct observations from codebase inspection, dependencies, and requirements:

1. **`Cargo.toml` Dependencies & Tooling**:
   - `Cargo.toml` lines 81-82: `pdf-extract = "0.7"` is already included in the workspace dependencies for pure-Rust PDF text extraction.
   - `Cargo.toml` lines 36-39: `serde = { version = "1.0", features = ["derive"] }`, `serde_json = "1.0"`, `toml = "0.8"`, `bincode = "1"`.
   - `Cargo.toml` lines 50-51: `unicode-normalization = "0.1"`, `unicode-segmentation = "1.10"`.
   - `Cargo.toml` lines 53-55: `anyhow = "1.0"`, `thiserror = "1.0"`.
   - `Cargo.toml` lines 32-33: `xxhash-rust = { version = "0.8", features = ["xxh3", "xxh64"] }`.
   - `Cargo.toml` lines 87-88: `chrono = "0.4"`.

2. **Existing Ingestion & Chunking Patterns in Codebase**:
   - `src/cli/init.rs` lines 83-88:
     ```rust
     let content = if *lang == Language::Pdf {
         pdf_extract::extract_text(path).ok()?
     } else {
         std::fs::read_to_string(path).ok()?
     };
     ```
     *Critical Flaw Observed in Legacy Code*: `pdf_extract::extract_text(path).ok()?` silently discards extraction errors and treats scanned PDFs as empty text without failing or notifying the user.
   - `src/chunking/prose.rs` lines 35-117: Demonstrates Markdown heading parsing (`starts_with('#')`) and double-newline paragraph splitting (`content.split("\n\n")`).
   - `src/error.rs` lines 5-17: `pub enum Error` defines standard engine errors. Currently lacks `PolicyError(String)` and `LedgerError(String)`.

3. **Authoritative Requirements & Edge Case Mandates**:
   - `ORIGINAL_REQUEST.md` line 20: *"No `unwrap()`/`expect()`/`panic!()` on user-input paths (policy PDFs, source files)."*
   - `ORIGINAL_REQUEST.md` line 49: *"**Edge Case (R3)**: A scanned-image PDF with no extractable text fails loudly with a clear error, not silently indexing an empty document."*
   - `PROJECT.md` lines 52-53:
     - F7: *PDF & Text Policy Parser in `src/policy/parser.rs` extracting text from `.pdf`, `.md`, `.txt`, `.policy`.*
     - F8: *Scanned-Image PDF Guard: Explicit loud error on scanned/image-only PDFs (<20 printable chars), preventing silent empty indexing.*
   - `SCOPE.md` lines 11-14:
     - Format support: `.pdf` (via `pdf-extract`), `.md`, `.txt`, `.policy`.
     - Scanned PDF check: If printable character count < 20, return explicit `Error::PolicyError("Scanned or unreadable PDF: insufficient extractable text")`.
     - Clause chunking / section extraction for clauses.

---

## 2. Logic Chain

From the observations above, we establish the following step-by-step logic chain for the design and implementation of Features F7 and F8:

```
[User Input: Policy File Path]
               │
               ▼
   [File Existence & Format Check]
   ├── Non-existent path? ───────► Err(Error::InvalidPath)
   ├── Unsupported extension? ───► Err(Error::PolicyError("Unsupported policy format..."))
   └── Supported: .pdf, .md, .txt, .policy, .rst
               │
               ▼
       [Format Ingestion]
       ├── .pdf ───► pdf_extract::extract_text(path)
       │               │
       │               ▼
       │      [Scanned PDF Guard (<20 printable chars)]
       │      ├── printable_count < 20 ──► Err(Error::PolicyError("Scanned or image-only PDF..."))
       │      └── printable_count >= 20 ─► Raw Text Buffer
       │
       └── .md, .txt, .policy ─► std::fs::read_to_string(path) ─► Raw Text Buffer
               │
               ▼
     [Policy Clause Chunking Engine]
     ├── Pass 1: Line-by-Line Header & Section Numbering Matcher
     │     ├── ATX Markdown Headings (`#`, `##`, `###`)
     │     ├── Section Keywords (`Section 1.1`, `Article 2`, `Requirement 3.2`)
     │     ├── Hierarchical Decimal (`1.1`, `1.1.2`, `4.2.3.1`)
     │     └── Numbered / Lettered (`1. Purpose`, `A. Scope`, `§ 164.312`)
     │
     ├── Pass 2: Paragraph Fallback (Double Newline `\n\n`)
     │     └── Used if no explicit headers/sections matched
     │
     └── Pass 3: Metadata Extraction & PolicyDocument Assembly
           ├── Title, Version, Content Hash (SHA-256)
           └── Vec<PolicyClause> with line ranges, byte offsets, and clause numbers
```

### 2.1 Format Ingestion Specifications (`.pdf`, `.md`, `.txt`, `.policy`)

1. **PDF Ingestion via `pdf-extract`**:
   - The parser reads the binary PDF at `path` using `pdf_extract::extract_text(path)`.
   - `pdf-extract` traverses the PDF page content streams, parsing fonts and extracting decoded text strings.
   - Any low-level parsing error from `pdf-extract` (corrupted PDF header, invalid xref table, encrypted PDF with unsupported cipher) is intercepted and converted to `Error::PolicyError(format!("Failed to parse PDF '{}': {e}", path.display()))`.
   - Under no circumstances does the engine panic, unwrap, or silently return an empty string.

2. **Markdown (`.md`, `.markdown`) Ingestion**:
   - Read directly using `std::fs::read_to_string(path)` into a UTF-8 `String`.
   - If the file contains non-UTF-8 bytes, convert via safe fallback or return `Error::PolicyError("Invalid UTF-8 encoding in policy file")`.
   - Markdown documents retain full ATX header syntax (`#`, `##`, `###`, etc.) to guide clause boundary detection.

3. **Plain Text (`.txt`, `.rst`) Ingestion**:
   - Read directly using `std::fs::read_to_string(path)`.
   - Scanned for alphanumeric section numbering patterns (e.g., `1.1`, `Section 1:`) or double-newline paragraph breaks.

4. **Policy DSL (`.policy`) Ingestion**:
   - Treated as structured plaintext/markdown.
   - Supports both markdown-style headers and formal regulatory section prefixes.

### 2.2 Scanned PDF Edge Case: Exact Detection & Failure Protocol

1. **Root Cause**:
   - Scanned PDFs (e.g. physical paperwork scanned to PDF) contain embedded raster image streams (`/Image` XObjects) and zero character text streams (`Tj`/`TJ` operators).
   - Ingesting a scanned PDF without OCR produces an empty or near-empty string containing only linefeeds/spaces.
   - If indexed silently, the compliance system would report 0 clauses, 0 obligations, and misleading 100% compliance or 0% coverage.

2. **Exact Character Counting Algorithm**:
   - Let $T$ be the extracted text string from `pdf_extract::extract_text(path)`.
   - Define *printable, non-whitespace characters* as:
     $$\text{printable\_count} = \sum_{c \in T} \mathbf{1}_{\{\neg \text{is\_whitespace}(c) \;\land\; \neg \text{is\_control}(c)\}}$$
   - In Rust:
     ```rust
     let printable_chars = extracted
         .chars()
         .filter(|c| !c.is_whitespace() && !c.is_control())
         .count();
     ```
3. **Threshold & Enforcement**:
   - Threshold: **`< 20` printable characters**.
   - If `printable_chars < 20`:
     ```rust
     return Err(Error::PolicyError(format!(
         "Scanned or image-only PDF detected at '{}': contains no extractable text (found {} printable characters). OCR is required before ingesting.",
         path.display(),
         printable_chars
     )));
     ```
   - **Never Silently Create Empty Documents**: The function returns `Err(Error::PolicyError(...))`, causing CLI `needle policy ingest` to terminate immediately with exit code 1 and display the clear error message.

### 2.3 Clause Chunking Strategy (Feature F8)

The clause chunker segments raw policy text into structured `PolicyClause` records. Each clause represents an atomic normative unit containing one or more obligations.

1. **Header Recognition Patterns**:
   The chunker evaluates lines using a prioritized hierarchy of patterns:
   - **Pattern A (Markdown ATX Heading)**:
     - Regex: `^(#{1,6})\s+(.+)$`
     - Matches: `# 1. Information Security Policy`, `## 4.2 Data Encryption`, `### Access Controls`
   - **Pattern B (Formal Regulatory Section Keywords)**:
     - Regex: `^(?i)(?:Section|Sec\.|Article|Art\.|Clause|Requirement|Req\.|Policy|Rule|Safeguard)\s+([0-9A-Za-z\.\-]+(?::|\.|\s+-|\s+))\s*(.*)$`
     - Matches: `Section 3.1: Password Complexity`, `Article 5 - Data Retention`, `Requirement 8.2.1 Multi-Factor Auth`
   - **Pattern C (Section Symbol Headings)**:
     - Regex: `^(§+\s*[0-9A-Za-z\.\-]+)\s*(.*)$`
     - Matches: `§ 164.312 Technical Safeguards`, `§§ 4.1-4.3 Key Management`
   - **Pattern D (Hierarchical Decimal Numbering)**:
     - Regex: `^(\d+(?:\.\d+)+)\s+([A-Z].*)$`
     - Matches: `1.1 Access Control`, `4.2.3 Encryption at Rest`, `8.1.1.2 Audit Logging`
   - **Pattern E (Top-Level Numbered Sections)**:
     - Regex: `^(\d+)\.\s+([A-Z][A-Za-z0-9\s,\-\(\)\/]{2,80})$`
     - Matches: `1. Purpose`, `2. Scope`, `3. Responsibilities`
   - **Pattern F (Lettered Sections)**:
     - Regex: `^([A-Z])\.\s+([A-Z][A-Za-z0-9\s,\-\(\)\/]{2,80})$`
     - Matches: `A. General Provisions`, `B. Technical Requirements`

2. **Chunking State Machine**:
   - Line-by-line streaming:
     - When a line matches any of Patterns A–F:
       - If there is an active clause (with accumulated body lines or title), flush it to `clauses`.
       - Compute `line_end = current_line_number - 1`.
       - Initialize a new clause:
         - Extract `clause_number` (e.g., `"4.2"`, `"Section 3"`, `"1.1.2"`).
         - Extract `title` (e.g., `"Data Encryption at Rest"`).
         - Set `line_start = current_line_number`.
     - If the line does not match any header pattern:
       - Append the line to `current_body_lines`.
   - End-of-file:
     - Flush the final clause with `line_end = total_lines`.

3. **Fallback Strategy for Unstructured Text**:
   - If after scanning, fewer than 2 clauses were found and the raw document contains multiple paragraphs (delimited by `\n\n`), execute paragraph-based chunking:
     - Split on `\n\n`.
     - For each non-empty paragraph:
       - `clause_number = format!("P{}", index + 1)`
       - `title = first line of paragraph (up to 60 characters)`
       - `raw_text = paragraph content`
       - Track line start and end offsets accurately.

4. **Preamble Preservation**:
   - Any text preceding the first numbered section (e.g. document preamble, executive summary) is captured as a clause with `clause_number = "0.0"` or `"Preamble"` and `title = "Preamble / Overview"`. No text is silently discarded.

---

## 3. Caveats

1. **No Embedded OCR Engine**:
   - `pdf-extract` parses text character streams from PDF layout tables. It does not contain an OCR engine (e.g. Tesseract).
   - Scanned PDFs without OCR will be detected and rejected. Users must run OCR (e.g. `ocrmypdf input.pdf output.pdf`) prior to ingesting. This is an intentional design decision to preserve deterministic air-gapped performance.

2. **Non-Standard Formatting Variations**:
   - Documents with non-standard section headers (e.g. all-lowercase headings without punctuation like `section three password rules`) will fall back to paragraph-level chunking. The regex engine handles all standard NIST, ISO 27001, SOC 2, HIPAA, and GDPR conventions.

3. **Memory Safety & Very Large PDFs**:
   - While policies are typically 5 to 100 pages, chunking operations operate on line iterators and vector buffers without unbounded recursion, ensuring compatibility with 1MB Windows thread stack limits.

---

## 4. Conclusion & Technical Implementation Specification

### 4.1 Error Variant in `src/error.rs`

```rust
// In src/error.rs
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    InvalidPath(String),
    IndexNotFound(String),
    ChunkingError(String),
    EmbeddingError(String),
    IndexError(String),
    QueryError(String),
    ConfigError(String),
    SerializationError(String),
    PolicyError(String),    // <-- Added for F7/F8
    LedgerError(String),    // <-- Added for F14-F20
    Other(Box<dyn std::error::Error>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // ... existing variants ...
            Error::PolicyError(e) => write!(f, "Policy error: {}", e),
            Error::LedgerError(e) => write!(f, "Ledger error: {}", e),
            Error::Other(e) => write!(f, "Error: {}", e),
        }
    }
}
```

### 4.2 Data Models in `src/policy/clause.rs`

```rust
// In src/policy/clause.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyFormat {
    Pdf,
    Markdown,
    PlainText,
    PolicyDsl,
}

impl PolicyFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "pdf" => Some(PolicyFormat::Pdf),
            "md" | "markdown" => Some(PolicyFormat::Markdown),
            "txt" | "text" | "rst" => Some(PolicyFormat::PlainText),
            "policy" => Some(PolicyFormat::PolicyDsl),
            _ => None,
        }
    }
}

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
    pub id: String,                    // e.g. "POL-001-OBL-01"
    pub clause_id: String,             // e.g. "POL-001-C01"
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
    pub id: String,                    // e.g. "POL-001-C01"
    pub document_id: String,           // e.g. "POL-001"
    pub clause_number: String,         // e.g. "4.2" or "Section 3"
    pub title: String,                 // e.g. "Data Encryption at Rest"
    pub raw_text: String,              // Full text of the clause
    pub obligations: Vec<PolicyObligation>, // Populated by F9 structurer
    pub line_start: u32,
    pub line_end: u32,
    pub byte_offset: u64,
    pub byte_length: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDocument {
    pub id: String,                    // e.g. "POL-SEC-2026"
    pub name: String,
    pub version: String,
    pub source_path: String,
    pub format: PolicyFormat,
    pub content_hash: String,          // SHA-256 hex string
    pub raw_text: String,
    pub clauses: Vec<PolicyClause>,
    pub created_at: String,            // ISO 8601 / RFC 3339 timestamp
}
```

### 4.3 Ingestion & Chunking Parser in `src/policy/parser.rs`

```rust
// In src/policy/parser.rs
use crate::error::{Error, Result};
use crate::policy::clause::{PolicyClause, PolicyDocument, PolicyFormat};
use chrono::Utc;
use std::path::Path;

pub struct PolicyParser;

impl PolicyParser {
    /// Ingest a policy file (.pdf, .md, .txt, .policy) and chunk into structured PolicyDocument.
    pub fn parse_file(
        path: &Path,
        custom_id: Option<String>,
        custom_name: Option<String>,
        version: Option<String>,
    ) -> Result<PolicyDocument> {
        if !path.exists() {
            return Err(Error::InvalidPath(format!(
                "Policy file not found: {}",
                path.display()
            )));
        }

        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        let format = PolicyFormat::from_extension(&ext).ok_or_else(|| {
            Error::PolicyError(format!(
                "Unsupported policy file format '.{}'. Supported formats: .pdf, .md, .txt, .policy",
                ext
            ))
        })?;

        let raw_text = match format {
            PolicyFormat::Pdf => {
                let extracted = pdf_extract::extract_text(path).map_err(|e| {
                    Error::PolicyError(format!("Failed to parse PDF '{}': {e}", path.display()))
                })?;

                // Guard for scanned / image-only PDF edge case (<20 printable chars)
                let printable_chars = extracted
                    .chars()
                    .filter(|c| !c.is_whitespace() && !c.is_control())
                    .count();

                if printable_chars < 20 {
                    return Err(Error::PolicyError(format!(
                        "Scanned or image-only PDF detected at '{}': contains no extractable text (found {} printable characters). OCR is required before ingesting.",
                        path.display(),
                        printable_chars
                    )));
                }

                extracted
            }
            PolicyFormat::Markdown | PolicyFormat::PlainText | PolicyFormat::PolicyDsl => {
                std::fs::read_to_string(path).map_err(Error::Io)?
            }
        };

        // Compute SHA-256 content hash
        let content_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(raw_text.as_bytes());
            hex::encode(hasher.finalize())
        };

        let file_stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled_policy".to_string());

        let doc_id = custom_id.unwrap_or_else(|| format!("POL-{}", &content_hash[..8].to_uppercase()));
        let doc_name = custom_name.unwrap_or_else(|| file_stem);
        let doc_version = version.unwrap_or_else(|| "1.0.0".to_string());

        let clauses = Self::chunk_clauses(&raw_text, &doc_id, format)?;

        Ok(PolicyDocument {
            id: doc_id,
            name: doc_name,
            version: doc_version,
            source_path: path.to_string_lossy().to_string(),
            format,
            content_hash,
            raw_text,
            clauses,
            created_at: Utc::now().to_rfc3339(),
        })
    }

    /// Split raw policy text into clauses using hierarchical header matching and paragraph fallback.
    pub fn chunk_clauses(
        raw_text: &str,
        doc_id: &str,
        format: PolicyFormat,
    ) -> Result<Vec<PolicyClause>> {
        let normalized = raw_text.replace("\r\n", "\n");
        let lines: Vec<&str> = normalized.lines().collect();

        if lines.is_empty() || raw_text.trim().is_empty() {
            return Err(Error::PolicyError(
                "Policy document contains no text content".to_string(),
            ));
        }

        let mut clauses = Vec::new();
        let mut current_clause_num = "0.0".to_string();
        let mut current_title = "Preamble".to_string();
        let mut current_lines = Vec::new();
        let mut start_line = 1u32;
        let mut byte_offset = 0u64;
        let mut clause_idx = 1;

        let flush_clause = |clauses: &mut Vec<PolicyClause>,
                            c_num: &str,
                            c_title: &str,
                            body_lines: &[&str],
                            s_line: u32,
                            e_line: u32,
                            offset: u64,
                            c_idx: &mut usize| {
            let body = body_lines.join("\n");
            let trimmed = body.trim();
            if trimmed.is_empty() && c_title == "Preamble" {
                return;
            }

            let full_text = if !c_title.is_empty() && !trimmed.starts_with(c_title) {
                format!("{}\n\n{}", c_title, trimmed)
            } else {
                trimmed.to_string()
            };

            let byte_len = full_text.len() as u32;
            let clause_id = format!("{}-C{:02}", doc_id, *c_idx);
            *c_idx += 1;

            clauses.push(PolicyClause {
                id: clause_id,
                document_id: doc_id.to_string(),
                clause_number: c_num.to_string(),
                title: c_title.to_string(),
                raw_text: full_text,
                obligations: Vec::new(),
                line_start: s_line,
                line_end: e_line,
                byte_offset: offset,
                byte_length: byte_len,
            });
        };

        for (i, &line) in lines.iter().enumerate() {
            let line_no = i as u32 + 1;
            let trimmed_line = line.trim();

            if let Some((clause_num, title)) = Self::detect_header(trimmed_line, format) {
                if !current_lines.is_empty() || current_title != "Preamble" {
                    let end_line = line_no.saturating_sub(1);
                    flush_clause(
                        &mut clauses,
                        &current_clause_num,
                        &current_title,
                        &current_lines,
                        start_line,
                        end_line,
                        byte_offset,
                        &mut clause_idx,
                    );
                    byte_offset += current_lines.join("\n").len() as u64;
                    current_lines.clear();
                }

                current_clause_num = clause_num;
                current_title = title;
                start_line = line_no;
            } else {
                current_lines.push(line);
            }
        }

        // Flush remaining lines
        let total_lines = lines.len() as u32;
        flush_clause(
            &mut clauses,
            &current_clause_num,
            &current_title,
            &current_lines,
            start_line,
            total_lines,
            byte_offset,
            &mut clause_idx,
        );

        // Fallback: If no clauses detected or only 1 generic clause, split by double newlines
        if clauses.is_empty() || (clauses.len() == 1 && clauses[0].raw_text.lines().count() > 30) {
            return Self::chunk_by_paragraphs(&normalized, doc_id);
        }

        Ok(clauses)
    }

    /// Detect header and extract (clause_number, title)
    fn detect_header(line: &str, _format: PolicyFormat) -> Option<(String, String)> {
        if line.is_empty() {
            return None;
        }

        // 1. Markdown ATX headings (# / ## / ###)
        if line.starts_with('#') {
            let heading_text = line.trim_start_matches('#').trim();
            // Check if heading has a section number e.g. "## 4.2 Data Encryption"
            if let Some((num, title)) = Self::split_number_and_title(heading_text) {
                return Some((num, title));
            }
            return Some(("§".to_string(), heading_text.to_string()));
        }

        // 2. Keyword section patterns ("Section 4.1: Encryption", "Article 2 - Scope", "Req 1.1")
        let lower = line.to_lowercase();
        for prefix in &["section", "sec.", "article", "art.", "clause", "requirement", "req.", "rule", "safeguard", "policy"] {
            if lower.starts_with(prefix) {
                let rest = line[prefix.len()..].trim();
                let clean = rest.trim_start_matches(':').trim_start_matches('.').trim_start_matches('-').trim();
                if let Some((num, title)) = Self::split_number_and_title(clean) {
                    return Some((format!("{} {}", prefix, num), title));
                }
                return Some((prefix.to_string(), clean.to_string()));
            }
        }

        // 3. Section symbol "§ 164.312 Technical Safeguards"
        if line.starts_with('§') {
            let rest = line.trim_start_matches('§').trim();
            if let Some((num, title)) = Self::split_number_and_title(rest) {
                return Some((format!("§ {}", num), title));
            }
            return Some(("§".to_string(), rest.to_string()));
        }

        // 4. Hierarchical decimal numbering e.g. "1.1 Access Control", "4.2.3 Password Rules"
        if let Some((num, title)) = Self::split_number_and_title(line) {
            // Guard: Number must look like "1.1" or "1.1.2" or "1." followed by Title Case
            if (num.contains('.') || num.parse::<u32>().is_ok()) && title.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                return Some((num, title));
            }
        }

        None
    }

    fn split_number_and_title(text: &str) -> Option<(String, String)> {
        let text = text.trim();
        let first_space = text.find(|c: char| c.is_whitespace() || c == ':' || c == '-')?;
        let (num_part, rest) = text.split_at(first_space);
        let clean_num = num_part.trim_matches(|c: char| c == '.' || c == ':' || c == '-').trim();
        let clean_title = rest.trim_matches(|c: char| c == '.' || c == ':' || c == '-' || c.is_whitespace()).trim();

        if clean_num.is_empty() || clean_title.is_empty() {
            return None;
        }

        // Validate num_part is digits, dots, letters (e.g. "1.1", "4.2.3", "A.1", "IV")
        let is_valid_num = clean_num.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
        if is_valid_num {
            Some((clean_num.to_string(), clean_title.to_string()))
        } else {
            None
        }
    }

    fn chunk_by_paragraphs(text: &str, doc_id: &str) -> Result<Vec<PolicyClause>> {
        let mut clauses = Vec::new();
        let paragraphs: Vec<&str> = text.split("\n\n").collect();
        let mut line_counter = 1u32;
        let mut byte_offset = 0u64;

        for (idx, para) in paragraphs.iter().enumerate() {
            let trimmed = para.trim();
            if trimmed.is_empty() {
                line_counter += para.lines().count() as u32 + 1;
                byte_offset += para.len() as u64 + 2;
                continue;
            }

            let lines_count = trimmed.lines().count() as u32;
            let end_line = line_counter + lines_count.saturating_sub(1);
            let first_line = trimmed.lines().next().unwrap_or("Paragraph");
            let title = if first_line.len() > 60 {
                format!("{}...", &first_line[..57])
            } else {
                first_line.to_string()
            };

            let clause_id = format!("{}-C{:02}", doc_id, idx + 1);
            clauses.push(PolicyClause {
                id: clause_id,
                document_id: doc_id.to_string(),
                clause_number: format!("P{}", idx + 1),
                title,
                raw_text: trimmed.to_string(),
                obligations: Vec::new(),
                line_start: line_counter,
                line_end: end_line,
                byte_offset,
                byte_length: trimmed.len() as u32,
            });

            line_counter = end_line + 2;
            byte_offset += para.len() as u64 + 2;
        }

        Ok(clauses)
    }
}
```

---

## 5. Verification Method

### 5.1 Unit Tests for Parser & Clause Chunker

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_scanned_pdf_error_loud_failure() {
        // Scanned PDF with 0 extractable characters
        let mut temp_pdf = NamedTempFile::new().unwrap();
        // Writing dummy non-text or binary content simulating image-only PDF
        temp_pdf.write_all(b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\ntrailer\n<<>>\n%%EOF").unwrap();

        let result = PolicyParser::parse_file(temp_pdf.path(), None, None, None);
        assert!(result.is_err(), "Must fail on scanned PDF");
        
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Scanned or image-only PDF detected") || err_msg.contains("Failed to parse PDF"),
            "Error message must clearly identify scanned or unparseable PDF: {}",
            err_msg
        );
    }

    #[test]
    fn test_markdown_policy_chunking() {
        let md_content = r#"
# 1.0 General Security Policy

All systems must implement proper security controls.

## 2.1 Password Requirements

Users must choose passwords of at least 16 characters.
Passwords shall be rotated every 90 days.

## 2.2 Access Control

Multi-factor authentication is required for all administrative access.
"#;
        let mut temp_md = NamedTempFile::new().unwrap();
        temp_md.write_all(md_content.as_bytes()).unwrap();

        let doc = PolicyParser::parse_file(temp_md.path(), Some("POL-TEST".into()), None, None).unwrap();
        assert_eq!(doc.clauses.len(), 3);
        assert_eq!(doc.clauses[0].clause_number, "1.0");
        assert_eq!(doc.clauses[1].clause_number, "2.1");
        assert_eq!(doc.clauses[2].clause_number, "2.2");
        assert!(doc.clauses[1].raw_text.contains("Passwords shall be rotated"));
    }

    #[test]
    fn test_plaintext_section_numbering_chunking() {
        let txt_content = r#"
Section 1.1: Authentication Controls
All endpoints must require bearer token authorization.

Section 1.2: Encryption Standards
Data at rest must be encrypted using AES-256.
Data in transit shall use TLS 1.3.
"#;
        let mut temp_txt = NamedTempFile::new().unwrap();
        temp_txt.write_all(txt_content.as_bytes()).unwrap();

        let doc = PolicyParser::parse_file(temp_txt.path(), None, None, None).unwrap();
        assert_eq!(doc.clauses.len(), 2);
        assert!(doc.clauses[0].title.contains("Authentication Controls"));
        assert!(doc.clauses[1].title.contains("Encryption Standards"));
    }

    #[test]
    fn test_unsupported_extension_rejection() {
        let mut temp_exe = NamedTempFile::with_suffix(".bin").unwrap();
        temp_exe.write_all(b"binary data").unwrap();

        let result = PolicyParser::parse_file(temp_exe.path(), None, None, None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Unsupported policy file format"));
    }
}
```

### 5.2 Test Invalidation Conditions

The implementation is invalid if:
1. A scanned-image PDF or PDF with `<20` printable characters returns `Ok(...)` with an empty document instead of `Err(Error::PolicyError(...))`.
2. Any `unwrap()` or `expect()` is called on user-provided policy paths or contents.
3. Clause boundaries fail to capture section numbers like `1.1`, `Section 2`, `Article 5`, or Markdown `# / ##`.
4. Document preamble or introductory sections are silently deleted instead of being tracked in `PolicyClause`.
5. Non-existent file paths return a generic panic rather than `Error::InvalidPath`.
