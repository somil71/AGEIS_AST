//! Obligation structuring engine supporting LLM extraction with deterministic heuristic fallback.

use crate::error::Result;
use crate::llm::LlmClient;
use crate::policy::clause::{ObligationType, PolicyClause, PolicyDocument, PolicyObligation, Severity};
use serde::Deserialize;
use std::collections::HashSet;

/// Coordinates obligation structuring using local/remote LLMs or deterministic heuristic rules.
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
    /// Create a structurer with an optional LLM client.
    pub fn new(llm_client: Option<LlmClient>) -> Self {
        Self {
            llm_client,
            heuristic_only: false,
        }
    }

    /// Create a structurer that runs exclusively in deterministic heuristic offline mode.
    pub fn heuristic_only() -> Self {
        Self {
            llm_client: None,
            heuristic_only: true,
        }
    }

    /// Process all clauses in a `PolicyDocument`, extracting and attaching structured obligations.
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

    /// Extract obligations from a single clause.
    pub async fn structure_clause(
        &self,
        clause: &PolicyClause,
        doc_id: &str,
        clause_index: usize,
    ) -> Vec<PolicyObligation> {
        self.structure_clause_internal(clause, doc_id, clause_index).await
    }

    async fn structure_clause_internal(
        &self,
        clause: &PolicyClause,
        doc_id: &str,
        clause_index: usize,
    ) -> Vec<PolicyObligation> {
        // Attempt LLM parsing if configured and not in heuristic-only mode
        if !self.heuristic_only {
            if let Some(client) = &self.llm_client {
                if let Ok(llm_obs) = self.try_llm_extraction(client, clause, doc_id, clause_index).await {
                    if !llm_obs.is_empty() {
                        return llm_obs;
                    }
                }
            }
        }

        // Deterministic heuristic rule fallback
        self.extract_heuristic(clause, doc_id, clause_index)
    }

    async fn try_llm_extraction(
        &self,
        client: &LlmClient,
        clause: &PolicyClause,
        doc_id: &str,
        clause_index: usize,
    ) -> std::result::Result<Vec<PolicyObligation>, String> {
        let system_prompt = "You are a cybersecurity policy parser. Extract atomic compliance obligations from the given policy text. Output a JSON array with schema: [{\"title\":\"...\",\"description\":\"...\",\"obligation_type\":\"must|must_not|should|may|required_if|prohibited_if\",\"severity\":\"critical|high|medium|low|informational\",\"target_entities\":[\"function\",\"endpoint\"],\"condition\":\"...\",\"action\":\"...\",\"target_keywords\":[\"kw1\",\"kw2\"],\"semantic_query\":\"...\",\"rule_criteria\":\"...\"}]. Return ONLY valid JSON.";

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

            let title = item
                .title
                .unwrap_or_else(|| format!("Obligation {}.{}", clause_index, i + 1));
            let description = item.description.unwrap_or_else(|| clause.raw_text.clone());
            let target_entities = item
                .target_entities
                .unwrap_or_else(|| vec!["function".into(), "endpoint".into()]);
            let target_keywords = item
                .target_keywords
                .unwrap_or_else(|| extract_lexical_keywords(&description));
            let semantic_query = item
                .semantic_query
                .unwrap_or_else(|| clean_semantic_query(&description));
            let rule_criteria = item
                .rule_criteria
                .unwrap_or_else(|| format!("Enforce {}: {}", obl_type, title));

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

    /// Deterministic heuristic rule extractor matching modal verbs, conditions, and risk domains.
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

        // Fallback for clauses without recognized modal verbs
        if obligations.is_empty() && !clause.raw_text.trim().is_empty() {
            let id = format!("{}-{:02}-OBL-01", doc_id, clause_index);
            let target_keywords = extract_lexical_keywords(&clause.raw_text);
            let target_entities = infer_target_entities(&clause.raw_text);
            obligations.push(PolicyObligation {
                id,
                clause_id: clause.id.clone(),
                title: clause.title.clone(),
                description: clause.raw_text.clone(),
                obligation_type: ObligationType::Should,
                severity: Severity::Low,
                target_entities,
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
// Helper Functions & Deterministic Parsers
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

pub fn split_into_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let normalized = text.replace("\r\n", "\n");

    for raw_line in normalized.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // If line is a list bullet or numbered item, treat as atomic sentence
        if line.starts_with('-')
            || line.starts_with('*')
            || line.starts_with('•')
            || (line.len() > 2 && line.chars().next().unwrap().is_ascii_digit() && (line.contains(". ") || line.contains(") ")))
        {
            let clean = line
                .trim_start_matches(|c: char| c == '-' || c == '*' || c == '•' || c.is_ascii_digit() || c == '.' || c == ')')
                .trim();
            if clean.len() > 5 {
                sentences.push(clean.to_string());
                continue;
            }
        }

        // Split standard prose by periods followed by space/capital letter or semicolon
        let parts = line.split(". ");
        for part in parts {
            let p = part.trim();
            if p.len() > 5 {
                sentences.push(p.to_string());
            }
        }
    }

    if sentences.is_empty() && !text.trim().is_empty() {
        sentences.push(text.trim().to_string());
    }

    sentences
}

/// Matches modal verbs and calculates severity based on deontic strength and domain risk terms.
pub fn classify_deontic_and_severity(sentence: &str) -> Option<(ObligationType, Severity)> {
    let s_lower = sentence.to_lowercase();

    // Check conditional triggers ("if", "when", "where", "unless", "in case")
    let has_condition = s_lower.starts_with("if ")
        || s_lower.contains(" if ")
        || s_lower.starts_with("when ")
        || s_lower.contains(" when ")
        || s_lower.starts_with("where ")
        || s_lower.contains(" where ")
        || s_lower.starts_with("unless ")
        || s_lower.contains(" unless ")
        || s_lower.contains("in case ")
        || s_lower.contains("whenever ")
        || s_lower.contains("provided that ");

    // Check prohibition phrases
    let has_prohibition = s_lower.contains("must not")
        || s_lower.contains("shall not")
        || s_lower.contains("cannot")
        || s_lower.contains("may not")
        || s_lower.contains("prohibited")
        || s_lower.contains("forbidden")
        || s_lower.contains("disallowed")
        || s_lower.contains("never");

    // Check affirmative requirement phrases
    let has_must = s_lower.contains("must")
        || s_lower.contains("shall")
        || s_lower.contains("required")
        || s_lower.contains("mandatory")
        || s_lower.contains("always")
        || s_lower.contains("compulsory")
        || s_lower.contains("enforce")
        || s_lower.contains("obligated");

    // Check recommendation phrases
    let has_should = s_lower.contains("should")
        || s_lower.contains("recommended")
        || s_lower.contains("strongly recommended")
        || s_lower.contains("ought to")
        || s_lower.contains("advise")
        || s_lower.contains("encouraged");

    // Check optional / permission phrases
    let has_may = s_lower.contains("may")
        || s_lower.contains("optional")
        || s_lower.contains("permitted")
        || s_lower.contains("allowed")
        || s_lower.contains("acceptable")
        || s_lower.contains("can");

    let obl_type = if has_condition && has_prohibition {
        ObligationType::ProhibitedIf
    } else if has_prohibition {
        ObligationType::MustNot
    } else if has_condition && has_must {
        ObligationType::RequiredIf
    } else if has_must {
        ObligationType::Must
    } else if has_should {
        ObligationType::Should
    } else if has_may {
        ObligationType::May
    } else {
        return None;
    };

    let severity = calculate_severity(&s_lower, &obl_type);
    Some((obl_type, severity))
}

pub fn calculate_severity(text: &str, obl_type: &ObligationType) -> Severity {
    // Explicit severity override checks
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
    if text.contains("[info]") || text.contains("severity: info") || text.contains("[informational]") {
        return Severity::Informational;
    }

    let critical_keywords = [
        "password", "plaintext", "secret", "private key", "private_key", "api_key",
        "api key", "auth_bypass", "rce", "remote code", "sql injection", "unauthenticated",
        "backdoor", "eval", "deserialization", "root", "privilege escalation", "tamper", "leak",
    ];

    let high_keywords = [
        "encrypt", "encryption", "tls", "https", "jwt", "session", "cookie", "cors",
        "csrf", "xss", "sanitize", "validate", "authorization", "rbac", "signature",
        "hmac", "sha256", "bcrypt", "argon2", "audit log", "pii", "token", "credentials",
    ];

    let medium_keywords = [
        "rate limit", "timeout", "error handling", "exception", "logging", "header",
        "content-type", "buffer", "memory limit", "cache", "versioning", "deprecation",
    ];

    let has_critical = critical_keywords.iter().any(|k| contains_keyword(text, k));
    let has_high = high_keywords.iter().any(|k| contains_keyword(text, k));
    let has_medium = medium_keywords.iter().any(|k| contains_keyword(text, k));

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

pub fn extract_condition_and_action(sentence: &str) -> (Option<String>, Option<String>) {
    let s_lower = sentence.to_lowercase();
    let condition_keywords = ["if ", "when ", "where ", "unless ", "provided that ", "in case "];

    let mut condition = None;
    for kw in &condition_keywords {
        if let Some(pos) = s_lower.find(kw) {
            let start = pos + kw.len();
            let remainder = &sentence[start..];
            // Condition spans until comma, 'then', or modal verb
            let end = remainder
                .find(|c: char| c == ',' || c == ';')
                .unwrap_or(remainder.len());
            let cond_str = remainder[..end].trim();
            if !cond_str.is_empty() {
                condition = Some(cond_str.to_string());
                break;
            }
        }
    }

    let modal_keywords = [
        "must not ", "shall not ", "cannot ", "may not ",
        "must ", "shall ", "should ", "may ", "required to ", "mandatory to "
    ];

    let mut action = None;
    for kw in &modal_keywords {
        if let Some(pos) = s_lower.find(kw) {
            let start = pos + kw.len();
            let remainder = &sentence[start..];
            let end = remainder
                .find(|c: char| c == '.' || c == ';' || c == '\n')
                .unwrap_or(remainder.len());
            let act_str = remainder[..end].trim();
            if !act_str.is_empty() {
                action = Some(act_str.to_string());
                break;
            }
        }
    }

    (condition, action)
}

pub fn extract_lexical_keywords(text: &str) -> Vec<String> {
    let stop_words: HashSet<&str> = [
        "the", "is", "at", "which", "on", "a", "an", "and", "or", "in", "to", "for", "with",
        "by", "that", "this", "all", "any", "from", "be", "as", "are", "of", "must", "shall",
        "should", "may", "not", "each", "every", "such", "when", "where", "if", "then", "into",
        "have", "has", "had", "can", "could", "will", "would", "its", "it's", "than", "their"
    ]
    .iter()
    .copied()
    .collect();

    let mut keywords = Vec::new();
    let cleaned = text
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { ' ' })
        .collect::<String>();

    for raw_word in cleaned.split_whitespace() {
        let w = raw_word.to_lowercase();
        let trimmed = w.trim_matches(|c: char| c == '_' || c == '-');
        if trimmed.len() >= 3 && !stop_words.contains(trimmed) && !keywords.contains(&trimmed.to_string()) {
            keywords.push(trimmed.to_string());
        }
    }

    keywords.truncate(8);
    keywords
}

pub fn infer_target_entities(text: &str) -> Vec<String> {
    let t = text.to_lowercase();
    let mut targets = Vec::new();

    if t.contains("endpoint")
        || t.contains("route")
        || t.contains("handler")
        || t.contains("api")
        || t.contains("http")
        || t.contains("request")
        || t.contains("response")
    {
        targets.push("endpoint".to_string());
    }
    if t.contains("function")
        || t.contains("method")
        || t.contains("procedure")
        || t.contains("routine")
        || t.contains("call")
    {
        targets.push("function".to_string());
    }
    if t.contains("struct")
        || t.contains("class")
        || t.contains("type")
        || t.contains("model")
        || t.contains("schema")
        || t.contains("entity")
    {
        targets.push("struct".to_string());
    }
    if t.contains("module")
        || t.contains("file")
        || t.contains("package")
        || t.contains("crate")
    {
        targets.push("module".to_string());
    }

    if targets.is_empty() {
        vec!["function".into(), "endpoint".into()]
    } else {
        targets
    }
}

pub fn clean_semantic_query(text: &str) -> String {
    let trimmed = text.trim();
    // Remove leading bullet points, numbers, or section prefixes
    let clean = trimmed
        .trim_start_matches(|c: char| c == '-' || c == '*' || c == '•' || c.is_ascii_digit() || c == '.' || c == ')' || c == ':')
        .trim();

    clean.to_string()
}

pub fn contains_keyword(text: &str, kw: &str) -> bool {
    if kw.contains(' ') || kw.contains('_') {
        text.contains(kw)
    } else {
        text.split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|w| w == kw)
    }
}

pub fn generate_obligation_title(sentence: &str, obl_type: &ObligationType) -> String {
    let clean = sentence
        .trim_start_matches(|c: char| c == '-' || c == '*' || c == '•' || c.is_ascii_digit() || c == '.' || c == ')' || c == ':')
        .trim();

    let words: Vec<&str> = clean.split_whitespace().take(8).collect();
    if words.is_empty() {
        format!("{obl_type} Requirement")
    } else {
        words.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::clause::PolicyFormat;

    #[test]
    fn test_heuristic_deontic_classification() {
        let (t1, s1) = classify_deontic_and_severity("All endpoints must require TLS 1.3 encryption.").unwrap();
        assert_eq!(t1, ObligationType::Must);
        assert_eq!(s1, Severity::High);

        let (t2, s2) = classify_deontic_and_severity("Applications must not store plaintext passwords in logs.").unwrap();
        assert_eq!(t2, ObligationType::MustNot);
        assert_eq!(s2, Severity::Critical);

        let (t3, s3) = classify_deontic_and_severity("Services should implement exponential backoff on retry.").unwrap();
        assert_eq!(t3, ObligationType::Should);
        assert_eq!(s3, Severity::Low);

        let (t4, s4) = classify_deontic_and_severity("Users may configure optional display themes.").unwrap();
        assert_eq!(t4, ObligationType::May);
        assert_eq!(s4, Severity::Informational);

        let (t5, _s5) = classify_deontic_and_severity("If user authentication fails, the server must lock the account.").unwrap();
        assert_eq!(t5, ObligationType::RequiredIf);

        let (t6, _s6) = classify_deontic_and_severity("When operating in production mode, debug routes must not be enabled.").unwrap();
        assert_eq!(t6, ObligationType::ProhibitedIf);
    }

    #[test]
    fn test_severity_calculation_keywords() {
        assert_eq!(
            calculate_severity("Users must not expose private key material.", &ObligationType::MustNot),
            Severity::Critical
        );
        assert_eq!(
            calculate_severity("Endpoints must enforce jwt signature verification.", &ObligationType::Must),
            Severity::High
        );
        assert_eq!(
            calculate_severity("Requests should include content-type header.", &ObligationType::Should),
            Severity::Low
        );
        assert_eq!(
            calculate_severity("Explicit tag [critical] must be honored.", &ObligationType::Should),
            Severity::Critical
        );
    }

    #[test]
    fn test_condition_and_action_extraction() {
        let text = "If handling financial transactions, the backend must verify jwt signature.";
        let (cond, act) = extract_condition_and_action(text);
        assert!(cond.is_some());
        assert!(cond.unwrap().contains("handling financial transactions"));
        assert!(act.is_some());
        assert!(act.unwrap().contains("verify jwt signature"));
    }

    #[test]
    fn test_target_entity_inference() {
        assert!(infer_target_entities("This route handler processes api requests.").contains(&"endpoint".to_string()));
        assert!(infer_target_entities("The helper function validates user input.").contains(&"function".to_string()));
        assert!(infer_target_entities("The UserRecord struct holds account state.").contains(&"struct".to_string()));
        assert!(infer_target_entities("The auth module encapsulates token parsing.").contains(&"module".to_string()));
    }

    #[test]
    fn test_lexical_keyword_extraction() {
        let text = "All database queries must sanitize sql parameters against injection attacks.";
        let kws = extract_lexical_keywords(text);
        assert!(kws.contains(&"database".to_string()));
        assert!(kws.contains(&"queries".to_string()));
        assert!(kws.contains(&"sanitize".to_string()));
        assert!(kws.contains(&"sql".to_string()));
        assert!(!kws.contains(&"the".to_string()));
        assert!(!kws.contains(&"all".to_string()));
    }

    #[tokio::test]
    async fn test_offline_document_structuring() {
        let mut doc = PolicyDocument {
            id: "POL-TEST-01".into(),
            name: "Test Policy".into(),
            version: "1.0.0".into(),
            source_path: "test.md".into(),
            format: PolicyFormat::Markdown,
            content_hash: "hash123".into(),
            raw_text: "## 1.0 Password Policy\nPasswords must be at least 16 characters.\n\n## 2.0 Encryption\nAll traffic must use TLS 1.3.".into(),
            clauses: vec![
                PolicyClause {
                    id: "POL-TEST-01-C01".into(),
                    document_id: "POL-TEST-01".into(),
                    clause_number: "1.0".into(),
                    title: "Password Policy".into(),
                    raw_text: "Passwords must be at least 16 characters. Users must not share passwords.".into(),
                    obligations: Vec::new(),
                    line_start: 1,
                    line_end: 2,
                    byte_offset: 0,
                    byte_length: 50,
                },
                PolicyClause {
                    id: "POL-TEST-01-C02".into(),
                    document_id: "POL-TEST-01".into(),
                    clause_number: "2.0".into(),
                    title: "Encryption".into(),
                    raw_text: "All traffic must use TLS 1.3.".into(),
                    obligations: Vec::new(),
                    line_start: 4,
                    line_end: 5,
                    byte_offset: 60,
                    byte_length: 30,
                },
            ],
            created_at: "2026-08-15T00:00:00Z".into(),
            metadata: Default::default(),
        };

        let structurer = ObligationStructurer::heuristic_only();
        let total = structurer.structure_document(&mut doc).await.unwrap();

        assert_eq!(total, 3);
        assert_eq!(doc.clauses[0].obligations.len(), 2);
        assert_eq!(doc.clauses[1].obligations.len(), 1);

        assert_eq!(doc.clauses[0].obligations[0].obligation_type, ObligationType::Must);
        assert_eq!(doc.clauses[0].obligations[1].obligation_type, ObligationType::MustNot);
        assert_eq!(doc.clauses[1].obligations[0].obligation_type, ObligationType::Must);
        assert_eq!(doc.clauses[1].obligations[0].severity, Severity::High);
    }
}
