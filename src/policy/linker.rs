//! Compliance Linker (M3) — links policy obligations to code graph nodes.
//!
//! Reuses the existing `QueryEngine` (BM25 + HNSW hybrid search) to find code
//! evidence for each policy obligation. Does NOT build a second search engine —
//! points the existing one at each obligation's extracted requirement text.

use crate::error::{Error, Result};
use crate::policy::clause::{ObligationType, PolicyClause, PolicyDocument, PolicyObligation};
use crate::query::QueryEngine;
use serde::{Deserialize, Serialize};

/// The compliance status of a single policy obligation against the code index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ComplianceStatus {
    Satisfied { evidence: Vec<EvidenceNode> },
    Violated { conflicting: Vec<EvidenceNode>, reason: String },
    NoImplementationFound,
}

/// A snippet of code evidence that matches an obligation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceNode {
    pub chunk_id: u64,
    pub file_path: String,
    pub line_start: u32,
    pub line_end: u32,
    /// RRF fusion score — NOT a probability, just a relative ranking signal.
    pub confidence: f32,
    /// First 200 chars of the matching chunk for quick inspection.
    pub snippet: String,
}

/// The result of running the compliance linker against one obligation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceLink {
    pub obligation_id: String,
    pub clause_id: String,
    pub clause_number: String,
    pub title: String,
    pub obligation_type: String,
    pub severity: String,
    pub status: ComplianceStatus,
}

/// The full compliance report for one policy document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub policy_id: String,
    pub policy_name: String,
    pub policy_version: String,
    pub generated_at: String,
    pub total_obligations: usize,
    pub satisfied: usize,
    pub violated: usize,
    pub no_evidence: usize,
    pub links: Vec<ComplianceLink>,
}

impl ComplianceReport {
    pub fn compliance_score(&self) -> f32 {
        if self.total_obligations == 0 { return 1.0; }
        self.satisfied as f32 / self.total_obligations as f32
    }
}

const EVIDENCE_THRESHOLD: f32 = 0.01;
const TOP_K: usize = 5;

/// Links every obligation in `doc` to code evidence in `engine`.
pub fn link_document(doc: &PolicyDocument, engine: &QueryEngine) -> Result<ComplianceReport> {
    let mut links = Vec::new();
    let mut satisfied = 0usize;
    let mut violated = 0usize;
    let mut no_evidence = 0usize;

    for clause in &doc.clauses {
        for obligation in &clause.obligations {
            let link = link_obligation(clause, obligation, engine)?;
            match &link.status {
                ComplianceStatus::Satisfied { .. } => satisfied += 1,
                ComplianceStatus::Violated { .. } => violated += 1,
                ComplianceStatus::NoImplementationFound => no_evidence += 1,
            }
            links.push(link);
        }
    }

    Ok(ComplianceReport {
        policy_id: doc.id.clone(),
        policy_name: doc.name.clone(),
        policy_version: doc.version.clone(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        total_obligations: links.len(),
        satisfied,
        violated,
        no_evidence,
        links,
    })
}

fn link_obligation(
    clause: &PolicyClause,
    obligation: &PolicyObligation,
    engine: &QueryEngine,
) -> Result<ComplianceLink> {
    let query = build_search_query(obligation);
    let (results, _timing) = engine
        .search(&query, TOP_K, None)
        .map_err(|e| Error::PolicyError(format!("Search failed for obligation '{}': {e}", obligation.id)))?;

    let evidence: Vec<EvidenceNode> = results
        .into_iter()
        .filter(|r| r.score > EVIDENCE_THRESHOLD)
        .map(|r| EvidenceNode {
            chunk_id: r.chunk_id,
            file_path: r.file_path.clone(),
            line_start: r.line_start,
            line_end: r.line_end,
            confidence: r.score,
            snippet: r.content.chars().take(200).collect(),
        })
        .collect();

    let status = if evidence.is_empty() {
        ComplianceStatus::NoImplementationFound
    } else {
        match check_numeric_violation(obligation, &evidence) {
            Some(reason) => ComplianceStatus::Violated { conflicting: evidence, reason },
            None => ComplianceStatus::Satisfied { evidence },
        }
    };

    Ok(ComplianceLink {
        obligation_id: obligation.id.clone(),
        clause_id: clause.id.clone(),
        clause_number: clause.clause_number.clone(),
        title: obligation.title.clone(),
        obligation_type: obligation.obligation_type.as_str().to_string(),
        severity: obligation.severity.as_str().to_string(),
        status,
    })
}

/// Public version of `link_obligation` for use by MCP tools.
pub fn link_obligation_pub(
    clause: &PolicyClause,
    obligation: &PolicyObligation,
    engine: &QueryEngine,
) -> Result<ComplianceLink> {
    link_obligation(clause, obligation, engine)
}

fn build_search_query(obligation: &PolicyObligation) -> String {
    let mut parts = vec![obligation.title.clone(), obligation.description.clone()];
    if let Some(action) = &obligation.action { parts.push(action.clone()); }
    if let Some(condition) = &obligation.condition { parts.push(condition.clone()); }
    // Also use the semantic_query field the structurer already generated
    if !obligation.semantic_query.is_empty() { parts.push(obligation.semantic_query.clone()); }
    parts.extend(obligation.target_keywords.iter().cloned());
    let keyword = match obligation.obligation_type {
        ObligationType::Must => "must implement required enforce",
        ObligationType::MustNot => "must not forbidden prohibited disable",
        ObligationType::Should => "should recommended best practice",
        ObligationType::May => "optional permitted allowed",
        ObligationType::RequiredIf => "conditional required when if",
        ObligationType::ProhibitedIf => "conditional prohibited when if not allowed",
    };
    parts.push(keyword.to_string());
    parts.join(" ")
}

/// Lightweight numeric violation checker.
///
/// Extracts numeric constraints from the obligation text (e.g. "7 years", "256 bits")
/// and compares them against any numeric literals found in the matched code snippets.
/// Returns `Some(reason)` if a contradiction is detected, `None` if all checks pass.
fn check_numeric_violation(obligation: &PolicyObligation, evidence: &[EvidenceNode]) -> Option<String> {
    let obligation_text = format!(
        "{} {} {}",
        obligation.title,
        obligation.description,
        obligation.action.as_deref().unwrap_or(""),
    ).to_lowercase();

    let unit_multipliers: &[(&str, f64)] = &[
        ("year", 365.0), ("month", 30.0), ("day", 1.0),
        ("bit", 1.0), ("byte", 1.0), ("char", 1.0), ("minute", 1.0 / 1440.0),
    ];

    // Extract (unit, required_days_or_units) from obligation
    let mut required: Option<(String, f64, f64)> = None; // (unit, raw_val, normalized)
    let tokens: Vec<&str> = obligation_text.split_whitespace().collect();
    'outer: for (i, token) in tokens.iter().enumerate() {
        if let Ok(n) = token.replace(',', "").parse::<f64>() {
            for (unit, mult) in unit_multipliers {
                if tokens.get(i + 1).map(|t| t.starts_with(unit)).unwrap_or(false) {
                    required = Some((unit.to_string(), n, n * mult));
                    break 'outer;
                }
            }
        }
    }

    let (req_unit, req_raw, req_norm) = required?;

    for node in evidence {
        let code = node.snippet.to_lowercase();
        let ctoks: Vec<&str> = code.split_whitespace().collect();
        for (i, token) in ctoks.iter().enumerate() {
            if let Ok(code_val) = token.replace(',', "").parse::<f64>() {
                for (unit, mult) in unit_multipliers {
                    if *unit == req_unit.as_str()
                        && ctoks.get(i + 1).map(|t| t.starts_with(unit)).unwrap_or(false)
                    {
                        let code_norm = code_val * mult;
                        if code_norm < req_norm * 0.95 {
                            return Some(format!(
                                "Code value ({code_val:.0} {req_unit}s) is below required minimum ({req_raw:.0} {req_unit}s) in policy"
                            ));
                        }
                    }
                }
            }
        }
    }
    None
}
