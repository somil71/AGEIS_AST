//! Policy data models: Documents, Clauses, Obligations, Deontic Types, and Severities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

/// Supported policy file formats.
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
        match ext.trim().trim_start_matches('.').to_lowercase().as_str() {
            "pdf" => Some(PolicyFormat::Pdf),
            "md" | "markdown" => Some(PolicyFormat::Markdown),
            "txt" | "text" | "rst" => Some(PolicyFormat::PlainText),
            "policy" => Some(PolicyFormat::PolicyDsl),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PolicyFormat::Pdf => "pdf",
            PolicyFormat::Markdown => "markdown",
            PolicyFormat::PlainText => "plain_text",
            PolicyFormat::PolicyDsl => "policy_dsl",
        }
    }
}

impl fmt::Display for PolicyFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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
            "should" | "recommended" | "ought_to" | "advise" => Some(ObligationType::Should),
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyClause {
    /// Unique clause identifier (e.g., "POL-001-C01").
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
    /// 1-based start line number in original file.
    pub line_start: u32,
    /// 1-based end line number in original file.
    pub line_end: u32,
    /// Byte offset in source file.
    pub byte_offset: u64,
    /// Byte length of clause.
    pub byte_length: u32,
}

/// An ingested policy document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyDocument {
    /// Unique document identifier (slug or hash).
    pub id: String,
    /// Policy title / name.
    pub name: String,
    /// Semantic version (e.g. "1.0.0").
    pub version: String,
    /// Original file path where document was ingested.
    pub source_path: String,
    /// Format of the source policy file.
    pub format: PolicyFormat,
    /// SHA-256 / xxHash content hash for deduplication and tamper-resistance.
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

    /// Retrieve all mandatory obligations (Must, MustNot, RequiredIf, ProhibitedIf).
    pub fn mandatory_obligations(&self) -> Vec<&PolicyObligation> {
        self.all_obligations()
            .into_iter()
            .filter(|o| o.obligation_type.is_mandatory())
            .collect()
    }

    /// Retrieve all prohibitions (MustNot, ProhibitedIf).
    pub fn prohibitions(&self) -> Vec<&PolicyObligation> {
        self.all_obligations()
            .into_iter()
            .filter(|o| o.obligation_type.is_prohibition())
            .collect()
    }

    /// Retrieve all critical severity obligations.
    pub fn critical_obligations(&self) -> Vec<&PolicyObligation> {
        self.all_obligations()
            .into_iter()
            .filter(|o| o.severity == Severity::Critical)
            .collect()
    }

    /// Parse a policy document directly from a file path using default settings.
    pub fn from_file(path: &Path) -> crate::Result<Self> {
        crate::policy::parser::PolicyParser::parse_file(path, None, None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obligation_type_properties() {
        assert!(ObligationType::Must.is_mandatory());
        assert!(ObligationType::MustNot.is_mandatory());
        assert!(ObligationType::RequiredIf.is_mandatory());
        assert!(ObligationType::ProhibitedIf.is_mandatory());
        assert!(!ObligationType::Should.is_mandatory());
        assert!(!ObligationType::May.is_mandatory());

        assert!(ObligationType::MustNot.is_prohibition());
        assert!(ObligationType::ProhibitedIf.is_prohibition());
        assert!(!ObligationType::Must.is_prohibition());
        assert!(!ObligationType::Should.is_prohibition());

        assert_eq!(ObligationType::from_str("must"), Some(ObligationType::Must));
        assert_eq!(ObligationType::from_str("shall"), Some(ObligationType::Must));
        assert_eq!(ObligationType::from_str("must_not"), Some(ObligationType::MustNot));
        assert_eq!(ObligationType::from_str("prohibited"), Some(ObligationType::MustNot));
        assert_eq!(ObligationType::from_str("required_if"), Some(ObligationType::RequiredIf));
        assert_eq!(ObligationType::from_str("prohibited_if"), Some(ObligationType::ProhibitedIf));
        assert_eq!(ObligationType::from_str("should"), Some(ObligationType::Should));
        assert_eq!(ObligationType::from_str("may"), Some(ObligationType::May));
    }

    #[test]
    fn test_severity_ordering_and_weights() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
        assert!(Severity::Low > Severity::Informational);

        assert_eq!(Severity::Critical.weight(), 25);
        assert_eq!(Severity::High.weight(), 10);
        assert_eq!(Severity::Medium.weight(), 5);
        assert_eq!(Severity::Low.weight(), 2);
        assert_eq!(Severity::Informational.weight(), 1);

        assert_eq!(Severity::from_str("critical"), Some(Severity::Critical));
        assert_eq!(Severity::from_str("crit"), Some(Severity::Critical));
        assert_eq!(Severity::from_str("high"), Some(Severity::High));
        assert_eq!(Severity::from_str("info"), Some(Severity::Informational));
    }

    #[test]
    fn test_policy_format_from_extension() {
        assert_eq!(PolicyFormat::from_extension("pdf"), Some(PolicyFormat::Pdf));
        assert_eq!(PolicyFormat::from_extension(".md"), Some(PolicyFormat::Markdown));
        assert_eq!(PolicyFormat::from_extension("markdown"), Some(PolicyFormat::Markdown));
        assert_eq!(PolicyFormat::from_extension("txt"), Some(PolicyFormat::PlainText));
        assert_eq!(PolicyFormat::from_extension("policy"), Some(PolicyFormat::PolicyDsl));
        assert_eq!(PolicyFormat::from_extension("exe"), None);
    }

    #[test]
    fn test_policy_document_helpers() {
        let obl1 = PolicyObligation {
            id: "OBL-1".into(),
            clause_id: "C-1".into(),
            title: "T1".into(),
            description: "Must do X".into(),
            obligation_type: ObligationType::Must,
            severity: Severity::Critical,
            target_entities: vec!["endpoint".into()],
            condition: None,
            action: Some("do X".into()),
            target_keywords: vec!["endpoint".into()],
            semantic_query: "Must do X".into(),
            rule_criteria: "Enforce X".into(),
        };

        let obl2 = PolicyObligation {
            id: "OBL-2".into(),
            clause_id: "C-1".into(),
            title: "T2".into(),
            description: "Must not do Y".into(),
            obligation_type: ObligationType::MustNot,
            severity: Severity::High,
            target_entities: vec!["function".into()],
            condition: None,
            action: Some("do Y".into()),
            target_keywords: vec!["function".into()],
            semantic_query: "Must not do Y".into(),
            rule_criteria: "Prohibit Y".into(),
        };

        let clause = PolicyClause {
            id: "C-1".into(),
            document_id: "DOC-1".into(),
            clause_number: "1.0".into(),
            title: "Security".into(),
            raw_text: "Text".into(),
            obligations: vec![obl1, obl2],
            line_start: 1,
            line_end: 5,
            byte_offset: 0,
            byte_length: 50,
        };

        let doc = PolicyDocument {
            id: "DOC-1".into(),
            name: "Test Doc".into(),
            version: "1.0.0".into(),
            source_path: "test.md".into(),
            format: PolicyFormat::Markdown,
            content_hash: "abcd".into(),
            raw_text: "Text".into(),
            clauses: vec![clause],
            created_at: "2026-08-15T00:00:00Z".into(),
            metadata: HashMap::new(),
        };

        assert_eq!(doc.total_obligations(), 2);
        assert_eq!(doc.all_obligations().len(), 2);
        assert_eq!(doc.mandatory_obligations().len(), 2);
        assert_eq!(doc.prohibitions().len(), 1);
        assert_eq!(doc.critical_obligations().len(), 1);
        assert_eq!(doc.critical_obligations()[0].id, "OBL-1");
    }
}
