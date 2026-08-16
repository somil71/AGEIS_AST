//! Cryptographic audit ledger verification engine and tamper localization.

use crate::ledger::block::{canonical_json_bytes, LedgerBlock};
use crate::ledger::crypto::{sha256_hex, verify_ed25519_signature, GENESIS_PREV_HASH};
use crate::Error;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Summary of a ledger verification run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationSummary {
    /// Total number of verified blocks in the chain
    pub total_blocks: u64,
    /// Whether the chain is cryptographically intact and unbroken
    pub is_valid: bool,
    /// The block hash of the latest block in the chain (None if empty chain)
    pub latest_block_hash: Option<String>,
}

/// Verifies the cryptographic integrity of a `.jsonl` audit ledger file.
///
/// Performs a 5-step verification process on every block:
/// 1. Parse JSON block structure
/// 2. Validate sequence continuity (0-indexed, strictly monotonic)
/// 3. Validate previous hash chaining (genesis starts at 64 zeroes)
/// 4. Validate payload hash against canonical JSON serialization
/// 5. Validate Ed25519 digital signature over signing preimage
/// 6. Validate block hash against block preimage
///
/// If any violation is encountered, verification halts and returns an error
/// localized to the exact sequence number: `"TAMPER DETECTED at sequence {N}: {reason}"`.
///
/// If the file does not exist or has 0 bytes, returns a clean summary with 0 blocks.
pub fn verify_ledger_file(ledger_path: &Path) -> Result<VerificationSummary, Error> {
    if !ledger_path.exists() {
        return Ok(VerificationSummary {
            total_blocks: 0,
            is_valid: true,
            latest_block_hash: None,
        });
    }

    let content = std::fs::read_to_string(ledger_path).map_err(Error::Io)?;
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

    if lines.is_empty() {
        return Ok(VerificationSummary {
            total_blocks: 0,
            is_valid: true,
            latest_block_hash: None,
        });
    }

    let mut prev_hash = GENESIS_PREV_HASH.to_string();

    for (line_idx, line) in lines.iter().enumerate() {
        let expected_sequence = line_idx as u64;
        // Step 1: Parse JSON block structure
        let block: LedgerBlock = serde_json::from_str(line).map_err(|e| {
            Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: invalid JSON block structure: {}",
                line_idx, e
            ))
        })?;

        // Step 2: Validate sequence continuity
        if block.sequence != expected_sequence {
            return Err(Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: sequence discontinuity (expected {}, found {})",
                block.sequence, expected_sequence, block.sequence
            )));
        }

        // Step 3: Validate previous block hash chaining
        if block.prev_hash != prev_hash {
            return Err(Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: prev_hash mismatch (expected {}, found {})",
                block.sequence, prev_hash, block.prev_hash
            )));
        }

        // Step 4: Validate deterministic payload hash
        let canonical_bytes = canonical_json_bytes(&block.payload)?;
        let computed_payload_hash = sha256_hex(&canonical_bytes);
        if block.payload_hash != computed_payload_hash {
            return Err(Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: payload_hash mismatch (expected {}, computed {})",
                block.sequence, block.payload_hash, computed_payload_hash
            )));
        }

        // Step 5: Validate Ed25519 signature
        let signing_preimage = LedgerBlock::signing_preimage(
            block.sequence,
            &block.timestamp,
            &block.prev_hash,
            &block.entry_type,
            &block.payload_hash,
        );

        let sig_valid = verify_ed25519_signature(
            &block.signer_public_key,
            signing_preimage.as_bytes(),
            &block.signature,
        )
        .unwrap_or(false);

        if !sig_valid {
            return Err(Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: invalid Ed25519 signature",
                block.sequence
            )));
        }

        // Step 6: Validate block hash
        let block_preimage = LedgerBlock::block_preimage(
            block.sequence,
            &block.timestamp,
            &block.prev_hash,
            &block.entry_type,
            &block.payload_hash,
            &block.signer_public_key,
            &block.signature,
        );
        let computed_block_hash = sha256_hex(block_preimage.as_bytes());
        if block.block_hash != computed_block_hash {
            return Err(Error::LedgerError(format!(
                "TAMPER DETECTED at sequence {}: block_hash mismatch (expected {}, computed {})",
                block.sequence, block.block_hash, computed_block_hash
            )));
        }

        // Advance chain state
        prev_hash = block.block_hash.clone();
    }

    Ok(VerificationSummary {
        total_blocks: lines.len() as u64,
        is_valid: true,
        latest_block_hash: Some(prev_hash),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_fresh_or_empty_ledger_verification() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        // 0-byte file
        let summary = verify_ledger_file(path).unwrap();
        assert_eq!(summary.total_blocks, 0);
        assert!(summary.is_valid);
        assert_eq!(summary.latest_block_hash, None);

        // Non-existent file
        let nonexistent = path.parent().unwrap().join("does_not_exist.jsonl");
        let summary2 = verify_ledger_file(&nonexistent).unwrap();
        assert_eq!(summary2.total_blocks, 0);
        assert!(summary2.is_valid);
        assert_eq!(summary2.latest_block_hash, None);
    }
}
