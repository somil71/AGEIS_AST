//! Cryptographic Audit Ledger Subsystem
//!
//! Provides an append-only, tamper-evident audit trail stored in `.needle/ledger/audit_chain.jsonl`.
//! Every entry is cryptographically hashed with SHA-256 and digitally signed using Ed25519.

pub mod block;
pub mod crypto;
pub mod keypair;
pub mod verifier;

pub use block::{canonical_json_bytes, canonicalize_json_value, EntryType, LedgerBlock};
pub use crypto::{
    sha256_digest, sha256_hex, sign_ed25519, verify_ed25519_signature, GENESIS_PREV_HASH,
};
pub use keypair::LedgerKeypair;
pub use verifier::{verify_ledger_file, VerificationSummary};

use crate::{Error, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Returns the default ledger directory: `<project_root>/.needle/ledger`
pub fn default_ledger_dir() -> PathBuf {
    let index_dir = crate::storage::Storage::default_index_dir();
    index_dir
        .parent()
        .map(|p| p.join("ledger"))
        .unwrap_or_else(|| PathBuf::from(".needle").join("ledger"))
}

/// Returns the default ledger file path: `<project_root>/.needle/ledger/audit_chain.jsonl`
pub fn default_ledger_path() -> PathBuf {
    default_ledger_dir().join("audit_chain.jsonl")
}

/// Returns the default private key path: `<project_root>/.needle/ledger/key.priv`
pub fn default_key_priv_path() -> PathBuf {
    default_ledger_dir().join("key.priv")
}

/// Returns the default public key path: `<project_root>/.needle/ledger/key.pub`
pub fn default_key_pub_path() -> PathBuf {
    default_ledger_dir().join("key.pub")
}

/// Appends a new audit record to the specified ledger file, signing it with the given keypair.
pub fn append_to_ledger(
    ledger_path: &Path,
    keypair: &LedgerKeypair,
    entry_type: EntryType,
    payload: serde_json::Value,
) -> Result<LedgerBlock> {
    // 1. Ensure parent directory exists
    if let Some(parent) = ledger_path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }

    // 2. Read existing chain to establish sequence and prev_hash
    let (sequence, prev_hash) = if ledger_path.exists() {
        let content = std::fs::read_to_string(ledger_path).map_err(Error::Io)?;
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        if let Some(last_line) = lines.last() {
            let last_block: LedgerBlock = serde_json::from_str(last_line).map_err(|e| {
                Error::LedgerError(format!(
                    "Cannot append: existing ledger contains corrupt block at tail: {}",
                    e
                ))
            })?;
            (last_block.sequence + 1, last_block.block_hash)
        } else {
            (0, GENESIS_PREV_HASH.to_string())
        }
    } else {
        (0, GENESIS_PREV_HASH.to_string())
    };

    // 3. Compute deterministic payload hash
    let canonical_payload = canonical_json_bytes(&payload)?;
    let payload_hash = sha256_hex(&canonical_payload);

    // 4. Generate RFC 3339 timestamp
    let timestamp = chrono::Utc::now().to_rfc3339();

    // 5. Construct signing preimage and sign
    let signing_preimage = LedgerBlock::signing_preimage(
        sequence,
        &timestamp,
        &prev_hash,
        &entry_type,
        &payload_hash,
    );
    let signature = sign_ed25519(&keypair.signing_key, signing_preimage.as_bytes());
    let signer_public_key = keypair.public_key_hex();

    // 6. Construct block preimage and compute block hash
    let block_preimage = LedgerBlock::block_preimage(
        sequence,
        &timestamp,
        &prev_hash,
        &entry_type,
        &payload_hash,
        &signer_public_key,
        &signature,
    );
    let block_hash = sha256_hex(block_preimage.as_bytes());

    // 7. Construct block object
    let block = LedgerBlock {
        sequence,
        timestamp,
        prev_hash,
        entry_type,
        payload_hash,
        payload,
        signer_public_key,
        signature,
        block_hash,
    };

    // 8. Append to .jsonl file
    let serialized_block =
        serde_json::to_string(&block).map_err(|e| Error::SerializationError(e.to_string()))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ledger_path)
        .map_err(Error::Io)?;

    writeln!(file, "{}", serialized_block).map_err(Error::Io)?;
    file.flush().map_err(Error::Io)?;

    Ok(block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_append_and_verify_chain() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        let keypair = LedgerKeypair::generate();

        let block0 = append_to_ledger(
            path,
            &keypair,
            EntryType::PolicyIngest,
            serde_json::json!({"policy": "POL-001", "version": "1.0"}),
        )
        .unwrap();
        assert_eq!(block0.sequence, 0);
        assert_eq!(block0.prev_hash, GENESIS_PREV_HASH);

        let block1 = append_to_ledger(
            path,
            &keypair,
            EntryType::ComplianceAudit,
            serde_json::json!({"score": 95, "violations": []}),
        )
        .unwrap();
        assert_eq!(block1.sequence, 1);
        assert_eq!(block1.prev_hash, block0.block_hash);

        let summary = verify_ledger_file(path).unwrap();
        assert_eq!(summary.total_blocks, 2);
        assert!(summary.is_valid);
        assert_eq!(summary.latest_block_hash, Some(block1.block_hash));
    }

    #[test]
    fn test_tamper_localization_payload_modification() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        let keypair = LedgerKeypair::generate();

        append_to_ledger(
            path,
            &keypair,
            EntryType::SecurityScan,
            serde_json::json!({"test": 1}),
        )
        .unwrap();
        append_to_ledger(
            path,
            &keypair,
            EntryType::ComplianceAudit,
            serde_json::json!({"test": 2}),
        )
        .unwrap();

        // Tamper with payload in block 1
        let content = std::fs::read_to_string(path).unwrap();
        let tampered = content.replace("\"test\":2", "\"test\":999");
        std::fs::write(path, tampered).unwrap();

        let err = verify_ledger_file(path).unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("TAMPER DETECTED at sequence 1: payload_hash mismatch"));
    }

    #[test]
    fn test_tamper_localization_broken_sequence() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        let keypair = LedgerKeypair::generate();

        append_to_ledger(
            path,
            &keypair,
            EntryType::SecurityScan,
            serde_json::json!({"test": 1}),
        )
        .unwrap();
        append_to_ledger(
            path,
            &keypair,
            EntryType::ComplianceAudit,
            serde_json::json!({"test": 2}),
        )
        .unwrap();

        // Tamper sequence number of block 1
        let content = std::fs::read_to_string(path).unwrap();
        let tampered = content.replace("\"sequence\":1", "\"sequence\":5");
        std::fs::write(path, tampered).unwrap();

        let err = verify_ledger_file(path).unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("TAMPER DETECTED at sequence 5: sequence discontinuity"));
    }

    #[test]
    fn test_tamper_localization_prev_hash_broken() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        let keypair = LedgerKeypair::generate();

        append_to_ledger(
            path,
            &keypair,
            EntryType::SecurityScan,
            serde_json::json!({"test": 1}),
        )
        .unwrap();
        append_to_ledger(
            path,
            &keypair,
            EntryType::ComplianceAudit,
            serde_json::json!({"test": 2}),
        )
        .unwrap();

        let content = std::fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        let block0: LedgerBlock = serde_json::from_str(lines[0]).unwrap();
        let mut block1: LedgerBlock = serde_json::from_str(lines[1]).unwrap();
        block1.prev_hash = "deadbeef".repeat(8);
        let tampered_lines = vec![
            serde_json::to_string(&block0).unwrap(),
            serde_json::to_string(&block1).unwrap(),
        ];
        std::fs::write(path, tampered_lines.join("\n")).unwrap();

        let err = verify_ledger_file(path).unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("TAMPER DETECTED at sequence 1: prev_hash mismatch"));
    }

    #[test]
    fn test_tamper_localization_invalid_signature() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        let keypair = LedgerKeypair::generate();

        append_to_ledger(
            path,
            &keypair,
            EntryType::SecurityScan,
            serde_json::json!({"test": 1}),
        )
        .unwrap();

        let content = std::fs::read_to_string(path).unwrap();
        let mut block0: LedgerBlock = serde_json::from_str(&content).unwrap();
        // Replace signature with a different valid 128 hex chars
        block0.signature = "aa".repeat(64);
        std::fs::write(path, serde_json::to_string(&block0).unwrap()).unwrap();

        let err = verify_ledger_file(path).unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("TAMPER DETECTED at sequence 0: invalid Ed25519 signature"));
    }
}
