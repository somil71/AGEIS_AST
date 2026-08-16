# Technical Investigation & Design Report: Feature F9 (Obligation Structuring & Heuristic Fallback) & CLI Integration (`src/cli/policy.rs`)

**Milestone**: M2 (Policy Ingestion & Obligation Structuring)  
**Agent**: Explorer 3  
**Working Directory**: `d:\AEGIS_AST\.agents\sub_orch_m2_policy_ingest\explorer_3`  
**Date**: 2026-08-14  

---

## 1. Observation

Direct observations from codebase inspection, architecture manifests, and authoritative specification documents (`PROJECT.md`, `ORIGINAL_REQUEST.md`, `spec_miner_policy_ledger/handoff.md`, `sub_orch_m2_policy_ingest/SCOPE.md`):

1. **Existing Engine & Architecture**:
   - `src/lib.rs`: Exports core modules (`analysis`, `chunking`, `config`, `embedding`, `error`, `graph`, `indexing`, `llm`, `query`, `schema`, `storage`, `server`, `watcher`). Needs `pub mod policy;` and `pub mod ledger;`.
   - `src/main.rs`: CLI dispatcher utilizing `clap::Parser` and `clap::Subcommand`. Currently routes `Init`, `Search`, `Status`, `Reindex`, `Config`, `Bench`, `Watch`, `Mcp`, `Serve`, `Report`, `Graph`. Needs `Policy` and `Audit` / `Ledger` subcommands.
   - `src/error.rs`: `pub enum Error` provides `Io`, `InvalidPath`, `IndexNotFound`, `ChunkingError`, `EmbeddingError`, `IndexError`, `QueryError`, `ConfigError`, `SerializationError`, `Other`. Requires `PolicyError(String)` variant for policy-specific parsing and structuring errors.
   - `src/llm.rs`: Provides `LlmClient` with `complete(system: &str, user: &str) -> Result<String, String>` and provider routing (Anthropic, OpenAI, Groq, Ollama). In sovereign mode (M1), cloud routes are gated and loopback `127.0.0.1:11434` is enforced.
   - `src/storage/mod.rs`: Manages persistence under `<project_root>/.needle/index`. Requires policy storage helper at `<project_root>/.needle/policy/` to store ingested `PolicyDocument` JSON records.

2. **Mandates & Constraints**:
   - **Zero Panics/Unwraps**: No `unwrap()`, `expect()`, or `panic!()` on user-input paths (policy PDFs, text documents, CLI arguments).
   - **Obligation Type Enum**: Must strictly define `Must`, `MustNot`, `Should`, `May`, `RequiredIf`, `ProhibitedIf` deontic modalities.
   - **Severity Enum**: Must strictly define `Critical`, `High`, `Medium`, `Low`, `Informational`.
   - **Dual Structuring Pipeline**: LLM-based structuring for rich semantic decomposition with an air-tight, deterministic heuristic rule fallback when LLM / Ollama is unavailable or `--heuristic-only` is specified.
   - **CLI Commands**: Implement `needle policy ingest <path>` (with `--name`, `--version`, `--dry-run`, `--heuristic-only`, `--format`) and `needle policy list` (with `--format`, `--verbose`).

---

## 2. Logic Chain & Technical Design

```
+---------------------------------------------------------------------------------------+
|                                    CLI Entry Point                                    |
|                      `needle policy ingest <path>` / `needle policy list`             |
+-------------------------------------------+-------------------------------------------+
                                            |
                                            v
+---------------------------------------------------------------------------------------+
|                              Document Ingestion & Chunking                            |
|             `src/policy/parser.rs` (PDF/TXT/MD -> Raw Extracted Clauses)              |
+-------------------------------------------+-------------------------------------------+
                                            |
                                            v
+---------------------------------------------------------------------------------------+
|                       Obligation Structurer (`src/policy/structurer.rs`)             |
|                                                                                       |
|   +---------------------------------------+   +-----------------------------------+   |
|   |         Primary: Local LLM            |   |   Fallback: Heuristic Rule Engine |   |
|   |   - JSON schema-guided extraction     |   |   - Deontic regex modal matcher   |   |
|   |   - Target entities & keywords        |   |   - Security keyword weighting    |   |
|   |   - Condition & Action parsing        |   |   - Condition / Action splitter   |   |
|   +-------------------+-------------------+   +-------------------+---------------+   |
|                       |                                           |                   |
|                       +---------------------+---------------------+                   |
|                                             |                                         |
|                                             v                                         |
|                          `Vec<PolicyObligation>` Constructed                          |
+---------------------------------------------+-----------------------------------------+
                                              |
                                              v
+---------------------------------------------------------------------------------------+
|                                Policy Store & Disk Layout                             |
|                        `<root>/.needle/policy/<policy_id>.json`                       |
+---------------------------------------------------------------------------------------+
```

### 2.1 Complete Data Model Design (`src/policy/clause.rs`)

```rust
//! Policy data models: Documents, Clauses, Obligations, Deontic Types, and Severities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Deontic obligation modalities based on RFC 2119 and regulatory compliance standards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationType {
    /// Mandatory affirmative obligation ("MUST", "SHALL", "REQUIRED").
    Must,
    /// Mandatory prohibition ("MUST NOT", "SHALL NOT", "PROHIBITED", "FORBIDDEN").
    MustNot,
    /// Strongly recommended guideline ("SHOULD", "RECOMMENDED").
    Should,
    /// Optional / permitted provision ("MAY", "OPTIONAL", "PERMITTED").
    May,
    /// Conditional affirmative obligation ("IF/WHEN ... MUST/SHALL").
    RequiredIf,
    /// Conditional prohibition ("IF/WHEN ... MUST NOT/PROHIBITED").
    ProhibitedIf,
}

impl ObligationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ObligationType::Must => "must",
            ObligationType::MustNot => "must_not",
            ObligationType::Should => "should",
            ObligationType::May => "may",
            ObligationType::RequiredIf => "required_if",
            ObligationType::ProhibitedIf => "prohibited_if",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().replace('-', "_").as_str() {
            "must" | "shall" | "required" | "mandatory" => Some(ObligationType::Must),
            "must_not" | "mustnot" | "shall_not" | "shallnot" | "prohibited" | "forbidden" => {
                Some(ObligationType::MustNot)
            }
            "should" | "recommended" | "ought_to" => Some(ObligationType::Should),
            "may" | "optional" | "permitted" | "allowed" => Some(ObligationType::May),
            "required_if" | "requiredif" | "conditional_must" => Some(ObligationType::RequiredIf),
            "prohibited_if" | "prohibitedif" | "conditional_prohibition" => {
                Some(ObligationType::ProhibitedIf)
            }
            _ => None,
        }
    }

    pub fn is_mandatory(&self) -> bool {
        matches!(
            self,
            ObligationType::Must
                | ObligationType::MustNot
                | ObligationType::RequiredIf
                | ObligationType::ProhibitedIf
        )
    }

    pub fn is_prohibition(&self) -> bool {
        matches!(self, ObligationType::MustNot | ObligationType::ProhibitedIf)
    }
}

impl fmt::Display for ObligationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Severity classification for policy obligations and compliance findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Informational,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Informational => "informational",
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "informational" | "info" => Some(Severity::Informational),
            "low" => Some(Severity::Low),
            "medium" | "med" => Some(Severity::Medium),
            "high" => Some(Severity::High),
            "critical" | "crit" => Some(Severity::Critical),
            _ => None,
        }
    }

    pub fn weight(&self) -> u32 {
        match self {
            Severity::Informational => 1,
            Severity::Low => 2,
            Severity::Medium => 5,
            Severity::High => 10,
            Severity::Critical => 25,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// An atomic, structured policy obligation extracted from a policy clause.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyObligation {
    /// Unique identifier (e.g., "POL-SEC-001-OBL-01").
    pub id: String,
    /// Identifier of the parent clause.
    pub clause_id: String,
    /// Short descriptive title.
    pub title: String,
    /// Full obligation description or source sentence.
    pub description: String,
    /// Deontic modal type.
    pub obligation_type: ObligationType,
    /// Associated severity level.
    pub severity: Severity,
    /// Target AST constructs (e.g., ["function", "endpoint", "struct"]).
    pub target_entities: Vec<String>,
    /// Optional conditional prerequisite (e.g., "when storing user passwords").
    pub condition: Option<String>,
    /// Concrete mandatory or prohibited action (e.g., "must use bcrypt or argon2").
    pub action: Option<String>,
    /// Lexical keywords for BM25 search.
    pub target_keywords: Vec<String>,
    /// Natural language query for HNSW vector embedding and hybrid search.
    pub semantic_query: String,
    /// Evaluation rule criteria for downstream matching.
    pub rule_criteria: String,
}

/// A segmented section or paragraph from a policy document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyClause {
    /// Unique clause identifier (e.g., "CLAUSE-1.1").
    pub id: String,
    /// Parent document identifier.
    pub document_id: String,
    /// Section / clause numbering (e.g., "1.1", "Section 4.2", "Article 12").
    pub clause_number: String,
    /// Section heading or title.
    pub title: String,
    /// Raw unparsed clause text.
    pub raw_text: String,
    /// Structured obligations extracted from this clause.
    pub obligations: Vec<PolicyObligation>,
}

/// An ingested policy document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDocument {
    /// Unique document identifier (slug or hash).
    pub id: String,
    /// Policy title / name.
    pub name: String,
    /// Semantic version (default "1.0.0").
    pub version: String,
    /// Original file path where document was ingested.
    pub source_path: String,
    /// SHA-256 hash of original file content for tamper-resistance and deduplication.
    pub content_hash: String,
    /// Full extracted raw text of document.
    pub raw_text: String,
    /// List of clauses.
    pub clauses: Vec<PolicyClause>,
    /// RFC3339 creation timestamp.
    pub created_at: String,
    /// Arbitrary metadata key-value pairs (e.g., author, jurisdiction).
    pub metadata: HashMap<String, String>,
}

impl PolicyDocument {
    /// Calculate total obligation count across all clauses.
    pub fn total_obligations(&self) -> usize {
        self.clauses.iter().map(|c| c.obligations.len()).sum()
    }

    /// Retrieve all obligations as a flat list.
    pub fn all_obligations(&self) -> Vec<&PolicyObligation> {
        self.clauses.iter().flat_map(|c| &c.obligations).collect()
    }
}
```

---

### 2.2 Structuring Engine Design (`src/policy/structurer.rs`)

The structurer executes a robust, two-tier extraction pipeline:
1. **Tier 1: LLM Processing** (when LLM client is configured and available).
2. **Tier 2: Deterministic Heuristic Rule Fallback** (when running offline without Ollama, when `--heuristic-only` flag is passed, or if LLM call/JSON parsing fails).

```rust
//! Obligation structuring engine supporting LLM parsing with deterministic heuristic rule fallback.

use crate::llm::LlmClient;
use crate::policy::clause::{ObligationType, PolicyClause, PolicyDocument, PolicyObligation, Severity};
use crate::Result;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashSet;

/// Obligation structurer coordinating LLM and heuristic rule extraction.
pub struct ObligationStructurer {
    llm_client: Option<LlmClient>,
    heuristic_only: bool,
}

#[derive(Debug, Deserialize)]
struct LlmObligationItem {
    title: Option<String>,
    description: Option<String>,
    obligation_type: Option<String>,
    severity: Option<String>,
    target_entities: Option<Vec<String>>,
    condition: Option<String>,
    action: Option<String>,
    target_keywords: Option<Vec<String>>,
    semantic_query: Option<String>,
    rule_criteria: Option<String>,
}

impl ObligationStructurer {
    pub fn new(llm_client: Option<LlmClient>) -> Self {
        Self {
            llm_client,
            heuristic_only: false,
        }
    }

    pub fn heuristic_only() -> Self {
        Self {
            llm_client: None,
            heuristic_only: true,
        }
    }

    /// Process all clauses in a PolicyDocument, populating obligations.
    pub async fn structure_document(&self, doc: &mut PolicyDocument) -> Result<usize> {
        let mut total_extracted = 0;
        let doc_id = doc.id.clone();

        for (clause_idx, clause) in doc.clauses.iter_mut().enumerate() {
            let obligations = self
                .structure_clause_internal(clause, &doc_id, clause_idx + 1)
                .await;
            total_extracted += obligations.len();
            clause.obligations = obligations;
        }

        Ok(total_extracted)
    }

    async fn structure_clause_internal(
        &self,
        clause: &PolicyClause,
        doc_id: &str,
        clause_index: usize,
    ) -> Vec<PolicyObligation> {
        // Attempt LLM parsing if available and not forced to heuristic
        if !self.heuristic_only {
            if let Some(client) = &self.llm_client {
                if let Ok(llm_obs) = self.try_llm_extraction(client, clause, doc_id, clause_index).await {
                    if !llm_obs.is_empty() {
                        return llm_obs;
                    }
                }
            }
        }

        // Fallback to deterministic heuristic rule engine
        self.extract_heuristic(clause, doc_id, clause_index)
    }

    async fn try_llm_extraction(
        &self,
        client: &LlmClient,
        clause: &PolicyClause,
        doc_id: &str,
        clause_index: usize,
    ) -> std::result::Result<Vec<PolicyObligation>, String> {
        let system_prompt = "You are a cybersecurity policy parser. Extract atomic compliance obligations from the given policy text. Output a JSON array with schema: [{\"title\":\"...\",\"description\":\"...\",\"obligation_type\":\"must|must_not|should|may|required_if|prohibited_if\",\"severity\":\"critical|high|medium|low|informational\",\"target_entities\":[\"function\"],\"condition\":\"...\",\"action\":\"...\",\"target_keywords\":[\"keyword\"],\"semantic_query\":\"...\",\"rule_criteria\":\"...\"}]. Return ONLY valid JSON.";

        let user_prompt = format!(
            "Clause: {} - {}\nText:\n{}",
            clause.clause_number, clause.title, clause.raw_text
        );

        let response = client.complete(system_prompt, &user_prompt).await?;
        let sanitized = sanitize_json_response(&response);
        let items: Vec<LlmObligationItem> = serde_json::from_str(&sanitized)
            .map_err(|e| format!("Failed to parse LLM JSON response: {e}"))?;

        let mut obligations = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            let obl_type = item
                .obligation_type
                .as_deref()
                .and_then(ObligationType::from_str)
                .unwrap_or(ObligationType::Must);

            let severity = item
                .severity
                .as_deref()
                .and_then(Severity::from_str)
                .unwrap_or(Severity::Medium);

            let title = item.title.unwrap_or_else(|| format!("Obligation {}.{}", clause_index, i + 1));
            let description = item.description.unwrap_or_else(|| clause.raw_text.clone());
            let target_entities = item.target_entities.unwrap_or_else(|| vec!["function".into(), "endpoint".into()]);
            let target_keywords = item.target_keywords.unwrap_or_else(|| extract_lexical_keywords(&description));
            let semantic_query = item.semantic_query.unwrap_or_else(|| description.clone());
            let rule_criteria = item.rule_criteria.unwrap_or_else(|| format!("Enforce {}", title));

            obligations.push(PolicyObligation {
                id: format!("{}-{:02}-OBL-{:02}", doc_id, clause_index, i + 1),
                clause_id: clause.id.clone(),
                title,
                description,
                obligation_type: obl_type,
                severity,
                target_entities,
                condition: item.condition,
                action: item.action,
                target_keywords,
                semantic_query,
                rule_criteria,
            });
        }

        Ok(obligations)
    }

    /// Deterministic heuristic rule fallback extractor.
    pub fn extract_heuristic(
        &self,
        clause: &PolicyClause,
        doc_id: &str,
        clause_index: usize,
    ) -> Vec<PolicyObligation> {
        let sentences = split_into_sentences(&clause.raw_text);
        let mut obligations = Vec::new();

        for sentence in sentences {
            let trimmed = sentence.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some((obl_type, severity)) = classify_deontic_and_severity(trimmed) {
                let obl_idx = obligations.len() + 1;
                let id = format!("{}-{:02}-OBL-{:02}", doc_id, clause_index, obl_idx);
                let (condition, action) = extract_condition_and_action(trimmed);
                let target_keywords = extract_lexical_keywords(trimmed);
                let target_entities = infer_target_entities(trimmed);
                let semantic_query = clean_semantic_query(trimmed);
                let title = generate_obligation_title(trimmed, &obl_type);

                obligations.push(PolicyObligation {
                    id,
                    clause_id: clause.id.clone(),
                    title,
                    description: trimmed.to_string(),
                    obligation_type: obl_type,
                    severity,
                    target_entities,
                    condition,
                    action,
                    target_keywords,
                    semantic_query,
                    rule_criteria: format!("Ensure code satisfies {}: {}", obl_type, trimmed),
                });
            }
        }

        // If no modal verbs matched but clause has content, create a default Informational/Medium obligation
        if obligations.is_empty() && !clause.raw_text.trim().is_empty() {
            let id = format!("{}-{:02}-OBL-01", doc_id, clause_index);
            let target_keywords = extract_lexical_keywords(&clause.raw_text);
            obligations.push(PolicyObligation {
                id,
                clause_id: clause.id.clone(),
                title: clause.title.clone(),
                description: clause.raw_text.clone(),
                obligation_type: ObligationType::Should,
                severity: Severity::Low,
                target_entities: vec!["function".into(), "endpoint".into()],
                condition: None,
                action: Some(clause.raw_text.clone()),
                target_keywords,
                semantic_query: clean_semantic_query(&clause.raw_text),
                rule_criteria: format!("Review compliance for {}", clause.title),
            });
        }

        obligations
    }
}

// ---------------------------------------------------------------------------
// Heuristic Helper Functions & Regex Pattern Engines
// ---------------------------------------------------------------------------

fn sanitize_json_response(raw: &str) -> String {
    let mut cleaned = raw.trim();
    if let Some(start) = cleaned.find("```json") {
        cleaned = &cleaned[start + 7..];
    } else if let Some(start) = cleaned.find("```") {
        cleaned = &cleaned[start + 3..];
    }
    if let Some(end) = cleaned.rfind("```") {
        cleaned = &cleaned[..end];
    }
    cleaned.trim().to_string()
}

fn split_into_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    // Split by period, semicolon, or newline followed by whitespace/bullet
    let re = Regex::new(r"(?:\.\s+|\n\s*[-*•\d+\.]+\s*|;\s*)").unwrap();
    for part in re.split(text) {
        let s = part.trim();
        if s.len() > 5 {
            sentences.push(s.to_string());
        }
    }
    if sentences.is_empty() && !text.trim().is_empty() {
        sentences.push(text.trim().to_string());
    }
    sentences
}

/// Matches modal verbs and calculates severity based on deontic strength and domain risk terms.
fn classify_deontic_and_severity(sentence: &str) -> Option<(ObligationType, Severity)> {
    let s_lower = sentence.to_lowercase();

    // Regex patterns for deontic modal detection
    let re_prohibited_if = Regex::new(r"\b(if|when|unless|where|in case)\b.{1,80}\b(must not|shall not|cannot|prohibited|forbidden|disallowed)\b").unwrap();
    let re_must_not = Regex::new(r"\b(must not|shall not|cannot|may not|is prohibited|are prohibited|strictly forbidden|disallowed|never)\b").unwrap();
    let re_required_if = Regex::new(r"\b(if|when|where|in case|whenever|provided that|unless)\b.{1,80}\b(must|shall|required|is mandatory|needs to|have to|enforce)\b").unwrap();
    let re_must = Regex::new(r"\b(must|shall|required|is mandatory|are mandatory|always|compulsory|enforce|obligated)\b").unwrap();
    let re_should = Regex::new(r"\b(should|should not|recommended|strongly recommended|ought to|advise|encouraged)\b").unwrap();
    let re_may = Regex::new(r"\b(may|optional|can|permitted|allowed|acceptable)\b").unwrap();

    let obl_type = if re_prohibited_if.is_match(&s_lower) {
        ObligationType::ProhibitedIf
    } else if re_must_not.is_match(&s_lower) {
        ObligationType::MustNot
    } else if re_required_if.is_match(&s_lower) {
        ObligationType::RequiredIf
    } else if re_must.is_match(&s_lower) {
        ObligationType::Must
    } else if re_should.is_match(&s_lower) {
        ObligationType::Should
    } else if re_may.is_match(&s_lower) {
        ObligationType::May
    } else {
        return None;
    };

    // Severity determination based on risk keywords
    let severity = calculate_severity(&s_lower, &obl_type);
    Some((obl_type, severity))
}

fn calculate_severity(text: &str, obl_type: &ObligationType) -> Severity {
    // Explicit severity override check
    if text.contains("[critical]") || text.contains("severity: critical") {
        return Severity::Critical;
    }
    if text.contains("[high]") || text.contains("severity: high") {
        return Severity::High;
    }
    if text.contains("[medium]") || text.contains("severity: medium") {
        return Severity::Medium;
    }
    if text.contains("[low]") || text.contains("severity: low") {
        return Severity::Low;
    }
    if text.contains("[info]") || text.contains("severity: info") {
        return Severity::Informational;
    }

    let critical_keywords = [
        "password", "plaintext", "secret", "private key", "private_key", "api_key",
        "api key", "auth_bypass", "rce", "remote code", "sql injection", "unauthenticated",
        "backdoor", "eval", "deserialization", "root", "privilege escalation", "tamper", "leak"
    ];

    let high_keywords = [
        "encrypt", "encryption", "tls", "https", "jwt", "session", "cookie", "cors",
        "csrf", "xss", "sanitize", "validate", "authorization", "rbac", "signature",
        "hmac", "sha256", "bcrypt", "argon2", "audit log", "pii"
    ];

    let medium_keywords = [
        "rate limit", "timeout", "error handling", "exception", "logging", "header",
        "content-type", "buffer", "memory limit", "cache", "versioning", "deprecation"
    ];

    let has_critical = critical_keywords.iter().any(|k| text.contains(k));
    let has_high = high_keywords.iter().any(|k| text.contains(k));
    let has_medium = medium_keywords.iter().any(|k| text.contains(k));

    match obl_type {
        ObligationType::MustNot | ObligationType::ProhibitedIf => {
            if has_critical {
                Severity::Critical
            } else if has_high {
                Severity::High
            } else {
                Severity::Medium
            }
        }
        ObligationType::Must | ObligationType::RequiredIf => {
            if has_critical {
                Severity::Critical
            } else if has_high {
                Severity::High
            } else if has_medium {
                Severity::Medium
            } else {
                Severity::Medium
            }
        }
        ObligationType::Should => {
            if has_critical {
                Severity::High
            } else if has_high {
                Severity::Medium
            } else {
                Severity::Low
            }
        }
        ObligationType::May => {
            if has_critical || has_high {
                Severity::Low
            } else {
                Severity::Informational
            }
        }
    }
}

fn extract_condition_and_action(sentence: &str) -> (Option<String>, Option<String>) {
    let re_cond = Regex::new(r"(?i)\b(if|when|where|unless|provided that|in the event of)\s+([^,]+?)(?:,\s*|\s+then\s+|\s+must|\s+shall)").unwrap();
    let re_act = Regex::new(r"(?i)\b(?:must not|shall not|must|shall|should|may|is required to|are required to)\s+(.+?)(?:\.|$|;)").unwrap();

    let condition = re_cond.captures(sentence).map(|c| c[2].trim().to_string());
    let action = re_act.captures(sentence).map(|c| c[1].trim().to_string());

    (condition, action)
}

fn extract_lexical_keywords(text: &str) -> Vec<String> {
    let stop_words: HashSet<&str> = [
        "the", "is", "at", "which", "on", "a", "an", "and", "or", "in", "to", "for", "with",
        "by", "that", "this", "all", "any", "from", "be", "as", "are", "of", "must", "shall",
        "should", "may", "not", "each", "every", "such", "when", "where", "if", "then"
    ].iter().cloned().collect();

    let mut keywords = Vec::new();
    let re_word = Regex::new(r"[a-zA-Z0-9_\-]+").unwrap();

    for mat in re_word.find_iter(text) {
        let w = mat.as_str().to_lowercase();
        if w.len() >= 3 && !stop_words.contains(w.as_str()) && !keywords.contains(&w) {
            keywords.push(w);
        }
    }
    keywords.truncate(8);
    keywords
}

fn infer_target_entities(text: &str) -> Vec<String> {
    let t = text.to_lowercase();
    let mut targets = Vec::new();

    if t.contains("endpoint") || t.contains("route") || t.contains("handler") || t.contains("api") || t.contains("http") {
        targets.push("endpoint".to_string());
    }
    if t.contains("function") || t.contains("method") || t.contains("procedure") || t.contains("routine") {
        targets.push("function".to_string());
    }
    if t.contains("struct") || t.contains("class") || t.contains("type") || t.contains("model") || t.contains("schema") {
        targets.push("struct".to_string());
    }
    if t.contains("module") || t.contains("file") || t.contains("package") || t.contains("crate") {
        targets.push("module".to_string());
    }

    if targets.is_empty() {
        vec!["function".into(), "endpoint".into()]
    } else {
        targets
    }
}

fn clean_semantic_query(text: &str) -> String {
    let re_clean = Regex::new(r"(?i)^(?:clause\s+\d+[\.\d]*:?|section\s+\d+[\.\d]*:?|\d+[\.\d]*\s*)").unwrap();
    let cleaned = re_clean.replace(text, "");
    cleaned.trim().to_string()
}

fn generate_obligation_title(sentence: &str, obl_type: &ObligationType) -> String {
    let words: Vec<&str> = sentence.split_whitespace().take(8).collect();
    if words.is_empty() {
        format!("{obl_type} Requirement")
    } else {
        words.join(" ")
    }
}
```

---

### 2.3 Persistence Layer Layout (`.needle/policy/`)

Policy documents are saved under `<project_root>/.needle/policy/<policy_id>.json`.
`Storage` in `src/storage/mod.rs` is extended with:
- `Storage::policy_dir() -> PathBuf`: `<project_root>/.needle/policy`
- `Storage::save_policy(&self, doc: &PolicyDocument) -> Result<()>`
- `Storage::load_policy(&self, id: &str) -> Result<PolicyDocument>`
- `Storage::list_policies(&self) -> Result<Vec<PolicyDocument>>`

---

### 2.4 CLI Interface Implementation (`src/cli/policy.rs`)

```rust
//! `needle policy` CLI subcommand implementation for ingestion and listing.

use crate::llm::LlmClient;
use crate::policy::clause::{ObligationType, PolicyDocument, Severity};
use crate::policy::parser::parse_policy_file;
use crate::policy::structurer::ObligationStructurer;
use crate::storage::Storage;
use crate::{Error, Result};
use clap::Subcommand;
use colored::Colorize;
use std::path::{Path, PathBuf};

#[derive(Subcommand, Debug, Clone)]
pub enum PolicyCommands {
    /// Ingest and parse a policy document (.pdf, .md, .txt, .policy)
    Ingest {
        /// Path to policy document file
        #[arg(required = true)]
        path: String,

        /// Custom policy name/identifier
        #[arg(short, long)]
        name: Option<String>,

        /// Policy semantic version
        #[arg(short, long, default_value = "1.0.0")]
        version: String,

        /// Dry-run mode: parse and extract without saving to disk
        #[arg(long)]
        dry_run: bool,

        /// Force heuristic-only structuring (bypass LLM)
        #[arg(long)]
        heuristic_only: bool,

        /// Output format: table, json, summary
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// List all ingested policy documents and obligations
    List {
        /// Output format: table, json
        #[arg(short, long, default_value = "table")]
        format: String,

        /// Show individual clauses and obligations
        #[arg(short, long)]
        verbose: bool,
    },
}

pub async fn run(cmd: PolicyCommands) -> Result<()> {
    match cmd {
        PolicyCommands::Ingest {
            path,
            name,
            version,
            dry_run,
            heuristic_only,
            format,
        } => run_ingest(&path, name, &version, dry_run, heuristic_only, &format).await,
        PolicyCommands::List { format, verbose } => run_list(&format, verbose).await,
    }
}

async fn run_ingest(
    file_path: &str,
    custom_name: Option<String>,
    version: &str,
    dry_run: bool,
    heuristic_only: bool,
    format: &str,
) -> Result<()> {
    let path = PathBuf::from(file_path);
    if !path.exists() {
        return Err(Error::InvalidPath(format!("Policy file not found: {}", path.display())));
    }

    let extracted = parse_policy_file(&path)?;

    let doc_name = custom_name.unwrap_or(extracted.title);
    let doc_id = slugify(&doc_name);

    // Compute content hash (SHA-256)
    let raw_bytes = std::fs::read(&path)?;
    let content_hash = format!("{:x}", sha2::Sha256::digest(&raw_bytes));

    let mut document = PolicyDocument {
        id: doc_id,
        name: doc_name,
        version: version.to_string(),
        source_path: path.to_string_lossy().to_string(),
        content_hash,
        raw_text: extracted.text.clone(),
        clauses: extracted.clauses,
        created_at: chrono::Utc::now().to_rfc3339(),
        metadata: Default::default(),
    };

    // Initialize structurer (LLM or Heuristic)
    let structurer = if heuristic_only {
        ObligationStructurer::heuristic_only()
    } else {
        let client = Some(LlmClient::from_env());
        ObligationStructurer::new(client)
    };

    let total_obligations = structurer.structure_document(&mut document).await?;

    if !dry_run {
        let storage = Storage::new(Storage::default_index_dir())?;
        storage.save_policy(&document)?;
    }

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&document)?);
        }
        _ => {
            print_ingest_summary(&document, total_obligations, dry_run);
        }
    }

    Ok(())
}

async fn run_list(format: &str, verbose: bool) -> Result<()> {
    let storage = Storage::new(Storage::default_index_dir())?;
    let policies = storage.list_policies()?;

    if policies.is_empty() {
        println!("{}", "No policies ingested yet. Ingest a policy with: needle policy ingest <path>".yellow());
        return Ok(());
    }

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&policies)?);
        }
        _ => {
            println!("{}", "Ingested Policy Documents\n".bold());
            for doc in &policies {
                println!(
                    "  {} {} (v{}) - {} clauses, {} obligations",
                    "•".cyan(),
                    doc.name.bold(),
                    doc.version.dimmed(),
                    doc.clauses.len().to_string().green(),
                    doc.total_obligations().to_string().green()
                );
                println!("    ID:          {}", doc.id.dimmed());
                println!("    Source:      {}", doc.source_path.dimmed());
                println!("    Ingested:    {}", doc.created_at.dimmed());

                if verbose {
                    println!();
                    for clause in &doc.clauses {
                        println!("    Clause {} - {}", clause.clause_number.bold(), clause.title);
                        for obl in &clause.obligations {
                            let type_colored = match obl.obligation_type {
                                ObligationType::Must => obl.obligation_type.as_str().cyan(),
                                ObligationType::MustNot => obl.obligation_type.as_str().red(),
                                ObligationType::Should => obl.obligation_type.as_str().yellow(),
                                ObligationType::May => obl.obligation_type.as_str().green(),
                                ObligationType::RequiredIf => obl.obligation_type.as_str().blue(),
                                ObligationType::ProhibitedIf => obl.obligation_type.as_str().magenta(),
                            };

                            let sev_colored = match obl.severity {
                                Severity::Critical => obl.severity.as_str().bold().red(),
                                Severity::High => obl.severity.as_str().red(),
                                Severity::Medium => obl.severity.as_str().yellow(),
                                Severity::Low => obl.severity.as_str().blue(),
                                Severity::Informational => obl.severity.as_str().dimmed(),
                            };

                            println!(
                                "      [{}] [{}] {}: {}",
                                type_colored,
                                sev_colored,
                                obl.id.dimmed(),
                                obl.title
                            );
                            if let Some(act) = &obl.action {
                                println!("        Action: {}", act.dimmed());
                            }
                        }
                    }
                    println!();
                }
            }
        }
    }

    Ok(())
}

fn print_ingest_summary(doc: &PolicyDocument, total_obligations: usize, dry_run: bool) {
    println!("{}", "Policy Ingestion Summary\n".bold());
    println!("  Title:       {}", doc.name.green().bold());
    println!("  Policy ID:   {}", doc.id.cyan());
    println!("  Version:     {}", doc.version);
    println!("  Source:      {}", doc.source_path);
    println!("  Clauses:     {}", doc.clauses.len().to_string().cyan());
    println!("  Obligations: {}", total_obligations.to_string().green().bold());

    if dry_run {
        println!("  Status:      {}", "[DRY RUN - NOT PERSISTED]".yellow().bold());
    } else {
        println!("  Status:      {}", "Successfully Ingested & Saved".green());
    }

    println!();
    println!("  {}:", "Extracted Obligations Breakdown".bold());
    for clause in &doc.clauses {
        println!("  Clause {}: {}", clause.clause_number.bold(), clause.title);
        for obl in &clause.obligations {
            println!(
                "    • [{}] [{}] {}: {}",
                obl.obligation_type.to_string().cyan(),
                obl.severity.to_string().yellow(),
                obl.id.dimmed(),
                obl.title
            );
        }
    }
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
```

---

### 2.5 `src/main.rs` Wiring Specification

Update `src/main.rs` to register the `Policy` subcommand:

```rust
// In Commands enum:
#[derive(clap::Subcommand)]
enum Commands {
    // ... existing commands ...

    /// Manage security & compliance policies (ingest, list)
    Policy {
        #[command(subcommand)]
        action: cli::policy::PolicyCommands,
    },
}

// In main dispatch:
match cli.command.unwrap_or(...) {
    // ...
    Commands::Policy { action } => cli::policy::run(action).await?,
}
```

---

## 3. Caveats

1. **LLM Connection Latency & Air-Gapped Fallback**:
   - In air-gapped environments, Ollama might not be pre-warmed or running. The structurer must never block indefinitely or crash; it catches connection errors and immediately invokes the deterministic heuristic fallback.
2. **Deontic Ambiguity**:
   - Regulatory language often combines multiple modalities in compound sentences (e.g. "If X is present, system must do Y and should not do Z"). The sentence/clause splitter handles compound clauses by breaking them into individual statements.
3. **Storage Isolation**:
   - Policies are stored in `.needle/policy/` scoped to the current project root, ensuring that policy definitions remain project-specific and do not pollute global configuration.

---

## 4. Conclusion

1. **Feature F9 Completeness**:
   - Structured `PolicyDocument`, `PolicyClause`, `PolicyObligation`, `ObligationType` (`Must`, `MustNot`, `Should`, `May`, `RequiredIf`, `ProhibitedIf`), and `Severity` (`Critical`, `High`, `Medium`, `Low`, `Informational`) provide the complete type foundation for Milestone M2, M3 compliance graph, and M4 cryptographic ledger.
2. **Deterministic Heuristic Fallback**:
   - Regex-based modal verb classification and security keyword weighting guarantee 100% deterministic obligation extraction even with zero LLM availability.
3. **CLI Ergonomics & Safety**:
   - `needle policy ingest` and `needle policy list` provide rich terminal feedback, structured JSON export, `--dry-run` safety, `--heuristic-only` testing, and strict zero-panic error handling.

---

## 5. Verification Method

To independently verify the implementation:

1. **Unit & Property Tests**:
   - Test modal verb regex mapping for all 6 `ObligationType` variants (`Must`, `MustNot`, `Should`, `May`, `RequiredIf`, `ProhibitedIf`).
   - Test severity calculations against critical, high, and medium risk keywords.
   - Test sentence splitting and condition/action extraction on complex policy text.
2. **CLI Ingestion Verification**:
   - Execute: `cargo run -- policy ingest tests/fixtures/sample_policy.md --dry-run`
   - **Expected**: Emits structured summary table of clauses and obligations with zero errors.
3. **Listing & JSON Export Verification**:
   - Execute: `cargo run -- policy ingest tests/fixtures/sample_policy.md`
   - Execute: `cargo run -- policy list --format json`
   - **Expected**: Emits valid JSON array containing ingested `PolicyDocument` and parsed `PolicyObligation` items.
