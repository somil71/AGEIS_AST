//! Policy parsing and clause chunking engine for PDF, Markdown, Plain Text, and Policy DSL.

use crate::error::{Error, Result};
use crate::policy::clause::{PolicyClause, PolicyDocument, PolicyFormat};
use chrono::Utc;
use std::collections::HashMap;
use std::path::Path;

/// High-level parser for policy documents across multiple file formats.
pub struct PolicyParser;

/// Intermediate result from raw text extraction.
#[derive(Debug, Clone)]
pub struct ExtractedPolicy {
    pub title: String,
    pub format: PolicyFormat,
    pub raw_text: String,
    pub clauses: Vec<PolicyClause>,
}

impl PolicyParser {
    /// Ingest and parse a policy file from disk into a structured `PolicyDocument`.
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

        let path_str = path.to_string_lossy().to_lowercase();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        let format = PolicyFormat::from_extension(&ext).or_else(|| {
            if path_str.ends_with(".pdf") || path_str.contains(".pdf") {
                Some(PolicyFormat::Pdf)
            } else if path_str.ends_with(".md") || path_str.contains(".md") || path_str.ends_with(".markdown") {
                Some(PolicyFormat::Markdown)
            } else if path_str.ends_with(".txt") || path_str.contains(".txt") || path_str.ends_with(".rst") {
                Some(PolicyFormat::PlainText)
            } else if path_str.ends_with(".policy") || path_str.contains(".policy") {
                Some(PolicyFormat::PolicyDsl)
            } else {
                None
            }
        }).ok_or_else(|| {
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

                // Scanned PDF guard: check printable non-whitespace and non-control characters
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
                let text = std::fs::read_to_string(path).map_err(Error::Io)?;
                if text.trim().is_empty() {
                    return Err(Error::PolicyError(format!(
                        "Policy file '{}' contains no text content",
                        path.display()
                    )));
                }
                text
            }
        };

        // Compute content hash (128-bit xxHash represented in hex)
        let content_hash = format!("{:032x}", xxhash_rust::xxh3::xxh3_128(raw_text.as_bytes()));

        let file_stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled_policy".to_string());

        let doc_name = custom_name.unwrap_or_else(|| {
            // Try extracting title from first markdown heading or use file stem
            Self::extract_document_title(&raw_text).unwrap_or(file_stem)
        });

        let doc_id = custom_id.unwrap_or_else(|| slugify(&doc_name));
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
            metadata: HashMap::new(),
        })
    }

    /// Parse a policy from an in-memory string with specified format and document ID.
    pub fn parse_str(
        raw_text: &str,
        doc_id: &str,
        doc_name: &str,
        format: PolicyFormat,
    ) -> Result<PolicyDocument> {
        if raw_text.trim().is_empty() {
            return Err(Error::PolicyError(
                "Policy text contains no content".to_string(),
            ));
        }

        let content_hash = format!("{:032x}", xxhash_rust::xxh3::xxh3_128(raw_text.as_bytes()));
        let clauses = Self::chunk_clauses(raw_text, doc_id, format)?;

        Ok(PolicyDocument {
            id: doc_id.to_string(),
            name: doc_name.to_string(),
            version: "1.0.0".to_string(),
            source_path: "<memory>".to_string(),
            format,
            content_hash,
            raw_text: raw_text.to_string(),
            clauses,
            created_at: Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
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
        let mut current_lines: Vec<&str> = Vec::new();
        let mut start_line = 1u32;
        let mut current_offset = 0u64;
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
            let trimmed_body = body.trim();
            if trimmed_body.is_empty() && c_title == "Preamble" {
                return;
            }

            let full_text = if !c_title.is_empty() && !trimmed_body.starts_with(c_title) && c_title != "Preamble" {
                if trimmed_body.is_empty() {
                    c_title.to_string()
                } else {
                    format!("{}\n\n{}", c_title, trimmed_body)
                }
            } else if trimmed_body.is_empty() {
                c_title.to_string()
            } else {
                trimmed_body.to_string()
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
                line_end: e_line.max(s_line),
                byte_offset: offset,
                byte_length: byte_len,
            });
        };

        for (i, &line) in lines.iter().enumerate() {
            let line_no = i as u32 + 1;
            let trimmed_line = line.trim();

            if let Some((clause_num, title)) = Self::detect_header(trimmed_line, format) {
                if !current_lines.is_empty() || (current_title != "Preamble" && !current_title.is_empty()) {
                    let end_line = line_no.saturating_sub(1);
                    flush_clause(
                        &mut clauses,
                        &current_clause_num,
                        &current_title,
                        &current_lines,
                        start_line,
                        end_line,
                        current_offset,
                        &mut clause_idx,
                    );
                    let body_len = current_lines.join("\n").len() as u64;
                    current_offset += body_len + 1;
                    current_lines.clear();
                }

                current_clause_num = clause_num;
                current_title = title;
                start_line = line_no;
            } else {
                current_lines.push(line);
            }
        }

        // Flush trailing clause
        let total_lines = lines.len() as u32;
        flush_clause(
            &mut clauses,
            &current_clause_num,
            &current_title,
            &current_lines,
            start_line,
            total_lines,
            current_offset,
            &mut clause_idx,
        );

        // Fallback: If no numbered clauses were detected and multiple paragraphs exist, split by paragraphs
        let has_numbered_clauses = clauses
            .iter()
            .any(|c| c.clause_number != "0.0" && c.clause_number != "#" && c.clause_number != "§");
        if (!has_numbered_clauses || clauses.len() <= 1) && normalized.contains("\n\n") {
            let paragraph_clauses = Self::chunk_by_paragraphs(&normalized, doc_id)?;
            if paragraph_clauses.len() > 1 {
                return Ok(paragraph_clauses);
            }
        }

        Ok(clauses)
    }

    /// Detect header and extract (clause_number, title)
    pub fn detect_header(line: &str, _format: PolicyFormat) -> Option<(String, String)> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        // 1. Markdown ATX headings (# / ## / ### / #### / ##### / ######)
        if trimmed.starts_with('#') {
            let heading_text = trimmed.trim_start_matches('#').trim();
            if heading_text.is_empty() {
                return None;
            }
            if let Some((num, title)) = Self::split_number_and_title(heading_text) {
                return Some((num, title));
            }
            return Some(("#".to_string(), heading_text.to_string()));
        }

        // 2. Section symbol headings "§ 164.312 Technical Safeguards" or "§§ 4.1-4.3"
        if trimmed.starts_with('§') {
            let rest = trimmed.trim_start_matches('§').trim();
            if let Some((num, title)) = Self::split_number_and_title(rest) {
                return Some((format!("§ {}", num), title));
            }
            return Some(("§".to_string(), rest.to_string()));
        }

        // 3. Keyword prefixes (Section, Sec., Article, Art., Clause, Requirement, Req., Rule, Safeguard, Policy)
        let lower = trimmed.to_lowercase();
        for prefix in &[
            "section", "sec.", "sec", "article", "art.", "art", "clause",
            "requirement", "req.", "req", "rule", "safeguard", "policy", "standard"
        ] {
            if lower.starts_with(prefix) {
                let rest = trimmed[prefix.len()..].trim();
                let clean = rest
                    .trim_start_matches(':')
                    .trim_start_matches('.')
                    .trim_start_matches('-')
                    .trim();

                if let Some((num, title)) = Self::split_number_and_title(clean) {
                    return Some((format!("{} {}", capitalize(prefix), num), title));
                }
                if !clean.is_empty() {
                    return Some((capitalize(prefix), clean.to_string()));
                }
            }
        }

        // 4. Hierarchical decimal numbering e.g. "1.1 Access Control", "4.2.3 Password Rules", "8.1.1.2 Audit"
        if let Some((num, title)) = Self::split_number_and_title(trimmed) {
            let has_digit = num.chars().any(|c| c.is_ascii_digit());
            let first_title_char = title.chars().next().unwrap_or(' ');
            if has_digit && (first_title_char.is_uppercase() || first_title_char == '"') {
                return Some((num, title));
            }
        }

        // 5. Lettered section patterns e.g. "A. General Provisions", "B. Technical Requirements"
        if trimmed.len() >= 4 {
            let first_char = trimmed.chars().next().unwrap_or(' ');
            let second_char = trimmed.chars().nth(1).unwrap_or(' ');
            if first_char.is_ascii_uppercase() && (second_char == '.' || second_char == ':') {
                let title = trimmed[2..].trim();
                if !title.is_empty() && title.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    return Some((first_char.to_string(), title.to_string()));
                }
            }
        }

        None
    }

    /// Split a string into a section number component and title component.
    pub fn split_number_and_title(text: &str) -> Option<(String, String)> {
        let text = text.trim();
        let first_sep = text.find(|c: char| c.is_whitespace() || c == ':' || c == '-');
        
        let (num_part, rest) = match first_sep {
            Some(pos) => text.split_at(pos),
            None => (text, ""), // No title, just a number
        };

        let clean_num = num_part.trim_matches(|c: char| c == '.' || c == ':' || c == '-').trim();
        let clean_title = rest
            .trim_matches(|c: char| c == '.' || c == ':' || c == '-' || c.is_whitespace())
            .trim();

        if clean_num.is_empty() {
            return None;
        }

        let has_digit = clean_num.chars().any(|c| c.is_ascii_digit());
        let is_letter_num = (clean_num.len() == 1 && clean_num.chars().next().unwrap().is_ascii_alphabetic())
            || matches!(
                clean_num.to_uppercase().as_str(),
                "I" | "II" | "III" | "IV" | "V" | "VI" | "VII" | "VIII" | "IX" | "X"
            );

        if (has_digit || is_letter_num || clean_num.starts_with('§'))
            && clean_num
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '§')
        {
            Some((clean_num.to_string(), clean_title.to_string()))
        } else {
            None
        }
    }

    /// Fallback chunking by double newline paragraphs when no formal headers are detected.
    fn chunk_by_paragraphs(text: &str, doc_id: &str) -> Result<Vec<PolicyClause>> {
        let mut clauses = Vec::new();
        let paragraphs: Vec<&str> = text.split("\n\n").collect();
        let mut line_counter = 1u32;
        let mut byte_offset = 0u64;

        for (idx, para) in paragraphs.iter().enumerate() {
            let trimmed = para.trim();
            if trimmed.is_empty() {
                let lines_in_blank = para.lines().count().max(1) as u32;
                line_counter += lines_in_blank;
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

        if clauses.is_empty() && !text.trim().is_empty() {
            let trimmed = text.trim();
            clauses.push(PolicyClause {
                id: format!("{}-C01", doc_id),
                document_id: doc_id.to_string(),
                clause_number: "P1".to_string(),
                title: "Policy Content".to_string(),
                raw_text: trimmed.to_string(),
                obligations: Vec::new(),
                line_start: 1,
                line_end: trimmed.lines().count() as u32,
                byte_offset: 0,
                byte_length: trimmed.len() as u32,
            });
        }

        Ok(clauses)
    }

    /// Extract document title from the first top-level Markdown heading if present.
    fn extract_document_title(raw_text: &str) -> Option<String> {
        for line in raw_text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("# ") {
                let title = trimmed.trim_start_matches('#').trim();
                if !title.is_empty() {
                    return Some(title.to_string());
                }
            }
        }
        None
    }
}

/// Helper function to parse a policy file with default settings.
pub fn parse_policy_file(path: &Path) -> Result<PolicyDocument> {
    PolicyParser::parse_file(path, None, None, None)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

pub fn slugify(s: &str) -> String {
    let slug = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();
    
    let parts: Vec<&str> = slug.split('-').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        "policy".to_string()
    } else {
        parts.join("-")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_scanned_pdf_error_loud_failure() {
        let mut temp_pdf = NamedTempFile::new().unwrap();
        // A minimal PDF stream that has zero/near-zero extractable text characters
        temp_pdf
            .write_all(b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\ntrailer\n<<>>\n%%EOF")
            .unwrap();

        let result = PolicyParser::parse_file(temp_pdf.path(), None, None, None);
        assert!(result.is_err(), "Must return Err on scanned or unextractable PDF");

        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("Scanned or image-only PDF")
                || err_str.contains("Failed to parse PDF")
                || err_str.contains("Policy error"),
            "Error message must be explicit: {}",
            err_str
        );
    }

    #[test]
    fn test_markdown_hierarchical_chunking() {
        let md_content = r#"
# Security and Compliance Policy

Preamble text explaining purpose and scope.

## 1.0 General Standards
All systems must adhere to strict security protocols.

## 2.1 Encryption at Rest
Databases storing sensitive information must use AES-256 encryption.

### 2.1.1 Key Rotation
Encryption keys shall be rotated at least every 90 days.
"#;
        let mut file = NamedTempFile::with_suffix(".md").unwrap();
        file.write_all(md_content.as_bytes()).unwrap();

        let doc = PolicyParser::parse_file(file.path(), Some("POL-TEST".into()), None, None).unwrap();
        assert_eq!(doc.id, "POL-TEST");
        assert_eq!(doc.format, PolicyFormat::Markdown);
        assert!(!doc.clauses.is_empty());

        let clause_numbers: Vec<&str> = doc.clauses.iter().map(|c| c.clause_number.as_str()).collect();
        assert!(clause_numbers.contains(&"1.0"));
        assert!(clause_numbers.contains(&"2.1"));
        assert!(clause_numbers.contains(&"2.1.1"));
    }

    #[test]
    fn test_plaintext_section_keywords_chunking() {
        let txt_content = r#"
Section 1.1: Authentication Requirements
All endpoints must require OAuth2 bearer token authentication.

Section 1.2: Password Complexity
Passwords must contain at least 16 characters including symbols.

Article 3 - Audit Logging
All critical transactions shall be logged to an immutable audit trail.
"#;
        let mut file = NamedTempFile::with_suffix(".txt").unwrap();
        file.write_all(txt_content.as_bytes()).unwrap();

        let doc = PolicyParser::parse_file(file.path(), None, None, None).unwrap();
        assert_eq!(doc.format, PolicyFormat::PlainText);
        assert_eq!(doc.clauses.len(), 3);
        assert!(doc.clauses[0].title.contains("Authentication Requirements"));
        assert!(doc.clauses[1].title.contains("Password Complexity"));
        assert!(doc.clauses[2].title.contains("Audit Logging"));
    }

    #[test]
    fn test_paragraph_fallback_chunking() {
        let unformatted_content = "This is the first paragraph describing general network rules and requirements for firewall policies.\n\nThis is the second paragraph describing administrative access controls and role-based permissions.\n\nThis is the third paragraph covering incident response timelines.";
        
        let doc = PolicyParser::parse_str(unformatted_content, "POL-UNFORMATTED", "Unformatted Policy", PolicyFormat::PlainText).unwrap();
        assert_eq!(doc.clauses.len(), 3);
        assert_eq!(doc.clauses[0].clause_number, "P1");
        assert_eq!(doc.clauses[1].clause_number, "P2");
        assert_eq!(doc.clauses[2].clause_number, "P3");
    }

    #[test]
    fn test_unsupported_format_rejection() {
        let mut file = NamedTempFile::with_suffix(".bin").unwrap();
        file.write_all(b"binary stream").unwrap();

        let result = PolicyParser::parse_file(file.path(), None, None, None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            Error::PolicyError(msg) => {
                assert!(msg.contains("Unsupported policy file format"));
            }
            _ => panic!("Expected PolicyError, got: {:?}", err),
        }
    }

    #[test]
    fn test_non_existent_file_rejection() {
        let fake_path = Path::new("non_existent_policy_file_xyz_12345.md");
        let result = PolicyParser::parse_file(fake_path, None, None, None);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidPath(msg) => {
                assert!(msg.contains("Policy file not found"));
            }
            other => panic!("Expected InvalidPath, got: {:?}", other),
        }
    }
}
