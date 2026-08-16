//! Comprehensive integration tests for the Cryptographic Audit Ledger Subsystem (Milestone M4).

use needle::ledger::{
    append_to_ledger, canonical_json_bytes, verify_ledger_file, EntryType, LedgerBlock,
    LedgerKeypair, GENESIS_PREV_HASH,
};
use tempfile::tempdir;

#[test]
fn test_genesis_hash_constant() {
    assert_eq!(
        GENESIS_PREV_HASH,
        "0000000000000000000000000000000000000000000000000000000000000000"
    );
    assert_eq!(GENESIS_PREV_HASH.len(), 64);
}

#[test]
fn test_canonical_json_nested_determinism() {
    let raw_json_1 = serde_json::json!({
        "zebra": 100,
        "apple": "pie",
        "nested": {
            "gamma": true,
            "alpha": [3, 2, 1],
            "beta": {
                "k2": "v2",
                "k1": "v1"
            }
        }
    });

    let raw_json_2 = serde_json::json!({
        "nested": {
            "beta": {
                "k1": "v1",
                "k2": "v2"
            },
            "gamma": true,
            "alpha": [3, 2, 1]
        },
        "apple": "pie",
        "zebra": 100
    });

    let b1 = canonical_json_bytes(&raw_json_1).unwrap();
    let b2 = canonical_json_bytes(&raw_json_2).unwrap();

    assert_eq!(b1, b2);
    let expected = r#"{"apple":"pie","nested":{"alpha":[3,2,1],"beta":{"k1":"v1","k2":"v2"},"gamma":true},"zebra":100}"#;
    assert_eq!(String::from_utf8(b1).unwrap(), expected);
}

#[test]
fn test_keypair_strict_redaction() {
    let keypair = LedgerKeypair::generate();
    let pub_hex = keypair.public_key_hex();
    let debug_repr = format!("{:?}", keypair);
    let display_repr = format!("{}", keypair);

    // Debug MUST contain redacted marker
    assert!(debug_repr.contains("[REDACTED PRIVATE KEY]"));
    assert!(debug_repr.contains(&pub_hex));

    // Display MUST format cleanly
    assert_eq!(display_repr, format!("LedgerKeypair(pubkey: {})", pub_hex));
}

#[test]
fn test_clean_empty_ledger_verification() {
    let dir = tempdir().unwrap();
    let missing_path = dir.path().join("missing_audit_chain.jsonl");

    // Case 1: Non-existent file
    let summary = verify_ledger_file(&missing_path).unwrap();
    assert_eq!(summary.total_blocks, 0);
    assert!(summary.is_valid);
    assert_eq!(summary.latest_block_hash, None);

    // Case 2: 0-byte file
    let empty_file = dir.path().join("empty.jsonl");
    std::fs::write(&empty_file, "").unwrap();
    let summary2 = verify_ledger_file(&empty_file).unwrap();
    assert_eq!(summary2.total_blocks, 0);
    assert!(summary2.is_valid);
    assert_eq!(summary2.latest_block_hash, None);

    // Case 3: Whitespace-only file
    let ws_file = dir.path().join("whitespace.jsonl");
    std::fs::write(&ws_file, "  \n  \t  \n").unwrap();
    let summary3 = verify_ledger_file(&ws_file).unwrap();
    assert_eq!(summary3.total_blocks, 0);
    assert!(summary3.is_valid);
    assert_eq!(summary3.latest_block_hash, None);
}

#[test]
fn test_multi_block_sequential_append_and_verification() {
    let dir = tempdir().unwrap();
    let ledger_path = dir.path().join("audit_chain.jsonl");
    let keypair = LedgerKeypair::generate();

    // Block 0: Genesis
    let block0 = append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::PolicyIngest,
        serde_json::json!({"policy_id": "SEC-001", "version": "1.0.0"}),
    )
    .unwrap();
    assert_eq!(block0.sequence, 0);
    assert_eq!(block0.prev_hash, GENESIS_PREV_HASH);
    assert_eq!(block0.entry_type, EntryType::PolicyIngest);

    // Block 1: Security scan
    let block1 = append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::SecurityScan,
        serde_json::json!({"target": "src/core", "vulnerabilities": 0}),
    )
    .unwrap();
    assert_eq!(block1.sequence, 1);
    assert_eq!(block1.prev_hash, block0.block_hash);

    // Block 2: Compliance audit
    let block2 = append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::ComplianceAudit,
        serde_json::json!({"score": 100, "status": "COMPLIANT"}),
    )
    .unwrap();
    assert_eq!(block2.sequence, 2);
    assert_eq!(block2.prev_hash, block1.block_hash);

    // Block 3: Codebase snapshot
    let block3 = append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::CodebaseSnapshot,
        serde_json::json!({"commit": "a1b2c3d4", "files": 42}),
    )
    .unwrap();
    assert_eq!(block3.sequence, 3);
    assert_eq!(block3.prev_hash, block2.block_hash);

    // Verify whole chain
    let summary = verify_ledger_file(&ledger_path).unwrap();
    assert_eq!(summary.total_blocks, 4);
    assert!(summary.is_valid);
    assert_eq!(summary.latest_block_hash, Some(block3.block_hash));
}

#[test]
fn test_tamper_localization_on_corrupt_json() {
    let dir = tempdir().unwrap();
    let ledger_path = dir.path().join("audit_chain.jsonl");
    let keypair = LedgerKeypair::generate();

    append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::SecurityScan,
        serde_json::json!({"test": 1}),
    )
    .unwrap();

    // Corrupt the line syntax
    std::fs::write(&ledger_path, "not a valid json block\n").unwrap();

    let err = verify_ledger_file(&ledger_path).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("TAMPER DETECTED at sequence 0: invalid JSON block structure"),
        "Unexpected error: {}",
        msg
    );
}

#[test]
fn test_tamper_localization_on_payload_mutation() {
    let dir = tempdir().unwrap();
    let ledger_path = dir.path().join("audit_chain.jsonl");
    let keypair = LedgerKeypair::generate();

    append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::SecurityScan,
        serde_json::json!({"count": 10}),
    )
    .unwrap();
    append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::ComplianceAudit,
        serde_json::json!({"score": 95}),
    )
    .unwrap();

    // Mutate block 1 payload
    let content = std::fs::read_to_string(&ledger_path).unwrap();
    let tampered = content.replace("\"score\":95", "\"score\":100");
    std::fs::write(&ledger_path, tampered).unwrap();

    let err = verify_ledger_file(&ledger_path).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("TAMPER DETECTED at sequence 1: payload_hash mismatch"),
        "Unexpected error: {}",
        msg
    );
}

#[test]
fn test_tamper_localization_on_signature_tampering() {
    let dir = tempdir().unwrap();
    let ledger_path = dir.path().join("audit_chain.jsonl");
    let keypair = LedgerKeypair::generate();

    append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::SecurityScan,
        serde_json::json!({"scan": "complete"}),
    )
    .unwrap();

    // Replace signature with dummy 128 hex chars
    let content = std::fs::read_to_string(&ledger_path).unwrap();
    let mut block: LedgerBlock = serde_json::from_str(&content).unwrap();
    block.signature = "0".repeat(128);
    std::fs::write(&ledger_path, serde_json::to_string(&block).unwrap()).unwrap();

    let err = verify_ledger_file(&ledger_path).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("TAMPER DETECTED at sequence 0: invalid Ed25519 signature"),
        "Unexpected error: {}",
        msg
    );
}

#[test]
fn test_tamper_localization_on_block_hash_tampering() {
    let dir = tempdir().unwrap();
    let ledger_path = dir.path().join("audit_chain.jsonl");
    let keypair = LedgerKeypair::generate();

    append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::SecurityScan,
        serde_json::json!({"scan": "complete"}),
    )
    .unwrap();

    // Replace block_hash with dummy 64 hex chars
    let content = std::fs::read_to_string(&ledger_path).unwrap();
    let mut block: LedgerBlock = serde_json::from_str(&content).unwrap();
    block.block_hash = "f".repeat(64);
    std::fs::write(&ledger_path, serde_json::to_string(&block).unwrap()).unwrap();

    let err = verify_ledger_file(&ledger_path).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("TAMPER DETECTED at sequence 0: block_hash mismatch"),
        "Unexpected error: {}",
        msg
    );
}

#[test]
fn test_tamper_localization_on_sequence_gap() {
    let dir = tempdir().unwrap();
    let ledger_path = dir.path().join("audit_chain.jsonl");
    let keypair = LedgerKeypair::generate();

    append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::SecurityScan,
        serde_json::json!({"item": 1}),
    )
    .unwrap();
    append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::SecurityScan,
        serde_json::json!({"item": 2}),
    )
    .unwrap();

    // Change block 1 sequence to 5
    let content = std::fs::read_to_string(&ledger_path).unwrap();
    let tampered = content.replace("\"sequence\":1", "\"sequence\":5");
    std::fs::write(&ledger_path, tampered).unwrap();

    let err = verify_ledger_file(&ledger_path).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("TAMPER DETECTED at sequence 5: sequence discontinuity"),
        "Unexpected error: {}",
        msg
    );
}

#[test]
fn test_tamper_localization_on_broken_chain_hash() {
    let dir = tempdir().unwrap();
    let ledger_path = dir.path().join("audit_chain.jsonl");
    let keypair = LedgerKeypair::generate();

    append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::SecurityScan,
        serde_json::json!({"item": 1}),
    )
    .unwrap();
    append_to_ledger(
        &ledger_path,
        &keypair,
        EntryType::SecurityScan,
        serde_json::json!({"item": 2}),
    )
    .unwrap();

    // Invert prev_hash in block 1
    let content = std::fs::read_to_string(&ledger_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let block0 = lines[0];
    let mut block1: LedgerBlock = serde_json::from_str(lines[1]).unwrap();
    block1.prev_hash = "a".repeat(64);
    let tampered = format!("{}\n{}", block0, serde_json::to_string(&block1).unwrap());
    std::fs::write(&ledger_path, tampered).unwrap();

    let err = verify_ledger_file(&ledger_path).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("TAMPER DETECTED at sequence 1: prev_hash mismatch"),
        "Unexpected error: {}",
        msg
    );
}
