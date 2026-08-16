# Technical Investigation & Design Report: Block Structure, Chaining, Verification & Tamper Localization (Milestone M4)

## 1. Observation

Direct observations from codebase inspection, scope contracts, and cryptographic ledger requirements:

1. **Workspace & Existing Files**:
   - `d:\AEGIS_AST\Cargo.toml` lines 1-125: Workspace contains `serde`, `serde_json`, `chrono`, `rand`, `thiserror`, `anyhow`. Missing crypto dependencies (`sha2`, `ed25519-dalek`, `hex`) being coordinated by Explorer 1.
   - `d:\AEGIS_AST\src\lib.rs` lines 6-21: Exports 13 core modules; requires `pub mod ledger;` to expose the audit ledger subsystem.
   - `d:\AEGIS_AST\src\error.rs` lines 6-17: Defines `Error` enum; needs `Error::LedgerError(String)` variant for tamper localization and ledger operational errors.
   - `d:\AEGIS_AST\.agents\sub_orch_m4_ledger\SCOPE.md` lines 26-42 & 56-76: Defines M4 architectural boundaries:
     - `block.rs`: `LedgerBlock`, `EntryType`, canonical JSON serialization, `signing_preimage` and `block_preimage`.
     - `verifier.rs`: `verify_ledger_file(&Path) -> Result<VerificationSummary, Error>`, clean empty chain handling, tamper localization.
     - `mod.rs`: `append_to_ledger(&Path, &LedgerKeypair, EntryType, serde_json::Value) -> Result<LedgerBlock, Error>`.
   - `d:\AEGIS_AST\.agents\spec_miner_policy_ledger\handoff.md` lines 360-586: Details reference structures, genesis hash (`"0000000000000000000000000000000000000000000000000000000000000000"`), and verbatim tamper error formats: `"TAMPER DETECTED at sequence {N}: {reason}"`.

2. **Core Invariants & Requirements**:
   - **Canonical JSON Encoding**: `payload_hash` must be calculated over deterministic, canonical JSON bytes where object keys are sorted lexicographically, floats/numbers/arrays follow invariant representation, and whitespace is compact.
   - **Preimage Construction**:
     - `signing_preimage = format!("{}:{}:{}:{:?}:{}", sequence, timestamp, prev_hash, entry_type, payload_hash)`
     - `block_preimage = format!("{}:{}:{}:{:?}:{}:{}:{}", sequence, timestamp, prev_hash, entry_type, payload_hash, signer_public_key, signature)`
   - **Chaining Invariant**:
     - Genesis block (`sequence == 0`): `prev_hash` is 64 zeroes (`"0000000000000000000000000000000000000000000000000000000000000000"`).
     - Block `N` (`sequence == N`): `prev_hash == block_{N-1}.block_hash`.
   - **Empty / 0-Byte Chain Invariant**:
     - Non-existent ledger file or 0-byte/whitespace-only file MUST verify cleanly without error, returning `VerificationSummary { total_blocks: 0, is_valid: true, latest_block_hash: None }`.
   - **Tamper Detection & Exact Sequence Localization**:
     - Any integrity failure (corrupt JSON, sequence discontinuity, prev_hash mismatch, payload modification, invalid signature, or block_hash mismatch) MUST return `Err(Error::LedgerError("TAMPER DETECTED at sequence {N}: {reason}".to_string()))`.
   - **Zero Panic Policy**: No `unwrap()`, `expect()`, or `panic!()` on any disk read, JSON parsing, or ledger verification paths.

---

## 2. Logic Chain

### 2.1 Block Data Structures (`src/ledger/block.rs`)

#### `EntryType` Enum
Represents the audit event classification:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    ComplianceAudit,
    SecurityScan,
    PolicyIngest,
    CodebaseSnapshot,
    SystemEvent,
}

impl EntryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntryType::ComplianceAudit => "compliance_audit",
            EntryType::SecurityScan => "security_scan",
            EntryType::PolicyIngest => "policy_ingest",
            EntryType::CodebaseSnapshot => "codebase_snapshot",
            EntryType::SystemEvent => "system_event",
        }
    }
}
```

#### `LedgerBlock` Struct
Represents a single tamper-evident block in `.needle/ledger/audit_chain.jsonl`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerBlock {
    /// 0-based strictly monotonic sequence number
    pub sequence: u64,
    /// RFC 3339 UTC timestamp (e.g. "2026-08-15T00:00:00Z")
    pub timestamp: String,
    /// SHA-256 hash of the previous block (or 64 zeroes for genesis block)
    pub prev_hash: String,
    /// Type/category of audit entry
    pub entry_type: EntryType,
    /// 64-char lowercase hex SHA-256 of canonical JSON payload
    pub payload_hash: String,
    /// Arbitrary JSON payload data (compliance report, scan findings, etc.)
    pub payload: serde_json::Value,
    /// 64-char lowercase hex Ed25519 public key of signer
    pub signer_public_key: String,
    /// 128-char lowercase hex Ed25519 digital signature over signing preimage
    pub signature: String,
    /// 64-char lowercase hex SHA-256 of block preimage
    pub block_hash: String,
}
```

---

### 2.2 Canonical JSON Serialization

To ensure deterministic hashing regardless of key order, whitespace, or nested object construction in `serde_json::Value`:

1. **Recursive Normalization**:
   Recursively traverse `serde_json::Value`. When encountering `Value::Object(map)`, insert all key-value pairs into a `std::collections::BTreeMap` to guarantee strict lexicographical key ordering.
2. **Compact Encoding**:
   `serde_json::to_vec` over the normalized value emits compact JSON without extra whitespace between keys, values, and delimiters.

```rust
use crate::{Error, Result};
use std::collections::BTreeMap;

/// Recursively sorts all JSON object keys in lexicographical order.
pub fn canonicalize_json_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k.clone(), canonicalize_json_value(v));
            }
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(canonicalize_json_value).collect())
        }
        other => other.clone(),
    }
}

/// Serializes arbitrary `serde_json::Value` into canonical, deterministic bytes.
pub fn canonical_json_bytes(value: &serde_json::Value) -> Result<Vec<u8>> {
    let canonical = canonicalize_json_value(value);
    serde_json::to_vec(&canonical).map_err(|e| Error::SerializationError(e.to_string()))
}
```

---

### 2.3 Preimage Construction

Preimages bind all block fields with `:` delimiters to prevent length extension and ambiguous field concatenation.

```rust
impl LedgerBlock {
    /// Signing Preimage format:
    /// "{sequence}:{timestamp}:{prev_hash}:{entry_type:?}:{payload_hash}"
    pub fn signing_preimage(
        sequence: u64,
        timestamp: &str,
        prev_hash: &str,
        entry_type: &EntryType,
        payload_hash: &str,
    ) -> String {
        format!("{}:{}:{}:{:?}:{}", sequence, timestamp, prev_hash, entry_type, payload_hash)
    }

    /// Block Preimage format:
    /// "{sequence}:{timestamp}:{prev_hash}:{entry_type:?}:{payload_hash}:{signer_public_key}:{signature}"
    pub fn block_preimage(
        sequence: u64,
        timestamp: &str,
        prev_hash: &str,
        entry_type: &EntryType,
        payload_hash: &str,
        signer_public_key: &str,
        signature: &str,
    ) -> String {
        format!(
            "{}:{}:{}:{:?}:{}:{}:{}",
            sequence, timestamp, prev_hash, entry_type, payload_hash, signer_public_key, signature
        )
    }
}
```

---

### 2.4 Verification Engine & Tamper Localization (`src/ledger/verifier.rs`)

#### Summary Type & Genesis Constants
```rust
pub const GENESIS_PREV_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub total_blocks: u64,
    pub is_valid: bool,
    pub latest_block_hash: Option<String>,
}
```

#### Verification Pipeline (Step-by-Step)

```
+-----------------------------------------------------------------------------------+
|                        verify_ledger_file(ledger_path)                            |
+-----------------------------------------------------------------------------------+
                                         |
                       File non-existent or 0-byte?
                                    /        \
                             YES   /          \   NO
                                  v            v
           Return Ok(total_blocks: 0,     Read lines (.jsonl)
                  is_valid: true,         Iterate line 0..N
                  latest_block_hash: None)     |
                                               v
                          +------------------------------------------+
                          | For each line with expected_seq, prev_h: |
                          |                                          |
                          | 1. Parse JSON -> LedgerBlock             |
                          |    Fail: "TAMPER DETECTED at sequence {N}|
                          |          : invalid JSON block structure" |
                          |                                          |
                          | 2. Check block.sequence == expected_seq  |
                          |    Fail: "TAMPER DETECTED at sequence {N}|
                          |          : sequence discontinuity"       |
                          |                                          |
                          | 3. Check block.prev_hash == prev_hash    |
                          |    Fail: "TAMPER DETECTED at sequence {N}|
                          |          : prev_hash mismatch"           |
                          |                                          |
                          | 4. Compute SHA-256(canonical(payload))   |
                          |    Fail: "TAMPER DETECTED at sequence {N}|
                          |          : payload_hash mismatch"        |
                          |                                          |
                          | 5. Verify Ed25519(pubkey, preimage, sig) |
                          |    Fail: "TAMPER DETECTED at sequence {N}|
                          |          : invalid Ed25519 signature"    |
                          |                                          |
                          | 6. Compute SHA-256(block_preimage)       |
                          |    Fail: "TAMPER DETECTED at sequence {N}|
                          |          : block_hash mismatch"          |
                          +------------------------------------------+
                                               |
                                        Advance state:
                                        prev_h = block.block_hash
                                        expected_seq += 1
                                               |
                                               v
                         Return Ok(total_blocks: N, is_valid: true,
                                   latest_block_hash: Some(prev_h))
```

#### Exact Implementation (`src/ledger/verifier.rs`):
```rust
use crate::ledger::block::{canonical_json_bytes, EntryType, LedgerBlock};
use crate::ledger::crypto::{sha256_hex, verify_ed25519_signature};
use crate::{Error, Result};
use std::path::Path;

pub fn verify_ledger_file(ledger_path: &Path) -> Result<VerificationSummary> {
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
    let mut expected_sequence = 0u64;

    for (line_idx, line) in lines.iter().enumerate() {
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
        ).unwrap_or(false);

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
        expected_sequence += 1;
    }

    Ok(VerificationSummary {
        total_blocks: lines.len() as u64,
        is_valid: true,
        latest_block_hash: Some(prev_hash),
    })
}
```

---

### 2.5 Ledger Append API & Module Root (`src/ledger/mod.rs`)

The ledger append API handles:
1. Automatic creation of parent directories (`.needle/ledger/`).
2. Sequence resumption from the last recorded block.
3. Cryptographic sealing (canonical hashing, signing, block hashing).
4. Atomic/append write to `.jsonl`.

```rust
pub mod block;
pub mod crypto;
pub mod keypair;
pub mod verifier;

pub use block::{canonical_json_bytes, canonicalize_json_value, EntryType, LedgerBlock};
pub use crypto::{sha256_hex, sign_ed25519, verify_ed25519_signature};
pub use keypair::LedgerKeypair;
pub use verifier::{verify_ledger_file, VerificationSummary, GENESIS_PREV_HASH};

use crate::{Error, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

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
    let serialized_block = serde_json::to_string(&block).map_err(|e| Error::SerializationError(e.to_string()))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ledger_path)
        .map_err(Error::Io)?;

    writeln!(file, "{}", serialized_block).map_err(Error::Io)?;
    file.flush().map_err(Error::Io)?;

    Ok(block)
}
```

---

## 3. Caveats

1. **Locking & Concurrency**:
   In high-concurrency environments with multiple writer processes, file appends could experience race conditions on sequence calculation. In single CLI invocations, file append mode is sufficient. If needed, a file lock mechanism (e.g. `fs2`) can be layered in.
2. **Float Representation in JSON**:
   Standard JSON canonicalization (RFC 8785) defines IEEE 754 float representation. In our use case, audit reports contain structured integers, strings, timestamps, and lists; using `canonicalize_json_value` with sorted keys guarantees 100% deterministic output.
3. **Timestamp Granularity**:
   Timestamp strings use RFC 3339 (`chrono::Utc::now().to_rfc3339()`). The exact string emitted during creation is sealed in the preimage, preventing any timestamp manipulation or timezone ambiguity.
4. **Keypair Redaction**:
   Explorer 1 handles `src/ledger/keypair.rs` ensuring `Debug` and `Display` never expose the private signing key.

---

## 4. Conclusion

The design for `block.rs`, `verifier.rs`, and `mod.rs` satisfies all requirements for Milestone M4:
- **Tamper Evident**: 5-layer cryptographic verification (sequence, prev_hash, canonical payload_hash, Ed25519 signature, block_hash).
- **Exact Tamper Localization**: Every failure mode halts immediately and outputs the exact sequence number where tampering occurred: `"TAMPER DETECTED at sequence {N}: {reason}"`.
- **Clean Fresh Verification**: 0-byte or non-existent ledgers verify cleanly with `total_blocks: 0`.
- **Deterministic**: Canonical JSON ordering ensures reproducible hash calculations.

---

## 5. Verification Method

Unit tests covering all core behaviors, edge cases, and tamper localization scenarios to be added in `src/ledger/` and integration tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::keypair::LedgerKeypair;
    use tempfile::NamedTempFile;

    #[test]
    fn test_fresh_ledger_verification() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        std::fs::remove_file(path).unwrap(); // ensure non-existent
        
        let summary = verify_ledger_file(path).unwrap();
        assert_eq!(summary.total_blocks, 0);
        assert!(summary.is_valid);
        assert_eq!(summary.latest_block_hash, None);
    }

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
        ).unwrap();
        assert_eq!(block0.sequence, 0);
        assert_eq!(block0.prev_hash, GENESIS_PREV_HASH);

        let block1 = append_to_ledger(
            path,
            &keypair,
            EntryType::ComplianceAudit,
            serde_json::json!({"score": 95, "violations": []}),
        ).unwrap();
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

        append_to_ledger(path, &keypair, EntryType::SecurityScan, serde_json::json!({"test": 1})).unwrap();
        append_to_ledger(path, &keypair, EntryType::ComplianceAudit, serde_json::json!({"test": 2})).unwrap();

        // Tamper with payload in block 1
        let content = std::fs::read_to_string(path).unwrap();
        let tampered = content.replace("\"test\": 2", "\"test\": 999");
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

        append_to_ledger(path, &keypair, EntryType::SecurityScan, serde_json::json!({"test": 1})).unwrap();
        append_to_ledger(path, &keypair, EntryType::ComplianceAudit, serde_json::json!({"test": 2})).unwrap();

        // Tamper sequence number of block 1
        let content = std::fs::read_to_string(path).unwrap();
        let tampered = content.replace("\"sequence\":1", "\"sequence\":5");
        std::fs::write(path, tampered).unwrap();

        let err = verify_ledger_file(path).unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("TAMPER DETECTED at sequence 5: sequence discontinuity"));
    }
}
```
