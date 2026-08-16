//! End-to-end integration tests for Milestone M2: Policy Ingestion, Parsing, Chunking & Obligation Structuring.

use needle::error::Error;
use needle::policy::clause::{ObligationType, PolicyFormat};
use needle::policy::parser::PolicyParser;
use needle::policy::structurer::ObligationStructurer;
use needle::storage::Storage;
use std::io::Write;
use std::path::Path;
use tempfile::{tempdir, NamedTempFile};

#[test]
fn test_scanned_pdf_loud_failure_edge_case() {
    let mut temp_pdf = NamedTempFile::with_suffix(".pdf").unwrap();
    // Simulate a scanned/image-only PDF with header and trailer but no text font operators or Tj text
    temp_pdf
        .write_all(b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\nxref\n0 4\n0000000000 65535 f \n0000000010 00000 n \n0000000060 00000 n \n0000000118 00000 n \ntrailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n198\n%%EOF")
        .unwrap();

    let result = PolicyParser::parse_file(temp_pdf.path(), None, None, None);
    assert!(
        result.is_err(),
        "Scanned PDF with < 20 printable characters MUST return Err"
    );

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("Scanned or image-only PDF detected") || err_msg.contains("Failed to parse PDF"),
        "Error message must explicitly call out scanned / unextractable PDF: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_full_markdown_policy_lifecycle() {
    let sample_policy = r#"
# HIPAA Security Rule — Technical Safeguards Policy

Policy document covering access control, audit controls, and integrity requirements.

## 1.0 Access Control (§ 164.312(a)(1))
The system must assign a unique name and/or number for identifying and tracking user identity.
Emergency access procedure must be established for obtaining necessary electronic protected health information.
Automatic logoff must be configured after 15 minutes of inactivity.

## 2.0 Audit Controls (§ 164.312(b))
Hardware, software, and/or procedural mechanisms must record and examine activity in information systems that contain or use ePHI.
Audit logs shall be protected against unauthorized modification or deletion.

## 3.0 Transmission Security (§ 164.312(e)(1))
All ePHI in transit must be encrypted using TLS 1.3.
Unencrypted transmission of confidential records is strictly prohibited.
"#;

    let mut file = NamedTempFile::with_suffix(".md").unwrap();
    file.write_all(sample_policy.as_bytes()).unwrap();

    // 1. Ingest & Parse
    let mut doc = PolicyParser::parse_file(file.path(), Some("HIPAA-TECH-01".into()), Some("HIPAA Technical Safeguards".into()), Some("2.0.0".into())).unwrap();
    assert_eq!(doc.id, "HIPAA-TECH-01");
    assert_eq!(doc.name, "HIPAA Technical Safeguards");
    assert_eq!(doc.version, "2.0.0");
    assert_eq!(doc.format, PolicyFormat::Markdown);
    assert!(!doc.clauses.is_empty());

    // 2. Structure Obligations offline
    let structurer = ObligationStructurer::heuristic_only();
    let count = structurer.structure_document(&mut doc).await.unwrap();
    assert!(count >= 5, "Expected at least 5 structured obligations, got {}", count);

    assert_eq!(doc.total_obligations(), count);
    assert!(!doc.mandatory_obligations().is_empty());
    assert!(!doc.prohibitions().is_empty());

    // 3. Storage Persistence Roundtrip
    let tmp_dir = tempdir().unwrap();
    let storage = Storage::new(tmp_dir.path().join("index")).unwrap();

    storage.save_policy(&doc).unwrap();

    let listed = storage.list_policies().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "HIPAA-TECH-01");
    assert_eq!(listed[0].total_obligations(), count);

    let loaded = storage.load_policy("HIPAA-TECH-01").unwrap();
    assert_eq!(loaded.id, doc.id);
    assert_eq!(loaded.content_hash, doc.content_hash);
    assert_eq!(loaded.clauses.len(), doc.clauses.len());
}

#[tokio::test]
async fn test_policy_dsl_format_parsing() {
    let dsl_policy = r#"
Section 1.1: Core Authentication
All external API routes must require Bearer token validation.

Section 1.2: Secret Management
Plaintext secrets and API keys must not be checked into version control.
If a secret is exposed, administrator must revoke the credential immediately.

Section 1.3: Rate Limiting
Public endpoints should implement rate limiting of 100 requests per minute.
"#;

    let mut file = NamedTempFile::with_suffix(".policy").unwrap();
    file.write_all(dsl_policy.as_bytes()).unwrap();

    let mut doc = PolicyParser::parse_file(file.path(), None, None, None).unwrap();
    assert_eq!(doc.format, PolicyFormat::PolicyDsl);
    assert_eq!(doc.clauses.len(), 3);

    let structurer = ObligationStructurer::heuristic_only();
    let total = structurer.structure_document(&mut doc).await.unwrap();
    assert_eq!(total, 4);

    let obls = doc.all_obligations();
    let has_must_not = obls.iter().any(|o| o.obligation_type == ObligationType::MustNot);
    let has_required_if = obls.iter().any(|o| o.obligation_type == ObligationType::RequiredIf);
    let has_should = obls.iter().any(|o| o.obligation_type == ObligationType::Should);

    assert!(has_must_not);
    assert!(has_required_if);
    assert!(has_should);
}

#[test]
fn test_unsupported_file_format_and_missing_path() {
    let bad_path = Path::new("non_existent_file.xyz");
    let res = PolicyParser::parse_file(bad_path, None, None, None);
    assert!(matches!(res.unwrap_err(), Error::InvalidPath(_)));

    let mut bad_ext_file = NamedTempFile::with_suffix(".docx").unwrap();
    bad_ext_file.write_all(b"dummy binary content").unwrap();
    let res2 = PolicyParser::parse_file(bad_ext_file.path(), None, None, None);
    assert!(matches!(res2.unwrap_err(), Error::PolicyError(_)));
}
