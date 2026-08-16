//! Block structures, entry types, canonical JSON serialization, and preimage generation.

use crate::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// Classification of ledger audit records.
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
    /// Returns the canonical snake_case string representation of the entry type.
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

impl FromStr for EntryType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "compliance_audit" | "complianceaudit" => Ok(EntryType::ComplianceAudit),
            "security_scan" | "securityscan" => Ok(EntryType::SecurityScan),
            "policy_ingest" | "policyingest" => Ok(EntryType::PolicyIngest),
            "codebase_snapshot" | "codebasesnapshot" => Ok(EntryType::CodebaseSnapshot),
            "system_event" | "systemevent" => Ok(EntryType::SystemEvent),
            _ => Err(Error::LedgerError(format!(
                "Invalid entry type: '{s}'. Valid types: compliance_audit, security_scan, policy_ingest, codebase_snapshot, system_event"
            ))),
        }
    }
}

impl fmt::Display for EntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A cryptographically chained, tamper-evident audit record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerBlock {
    /// 0-based strictly monotonic sequence number
    pub sequence: u64,
    /// RFC 3339 UTC timestamp
    pub timestamp: String,
    /// SHA-256 hash of the previous block (or 64 zeroes for genesis block)
    pub prev_hash: String,
    /// Classification of the audit record
    pub entry_type: EntryType,
    /// 64-character lowercase hex SHA-256 hash of the canonicalized JSON payload
    pub payload_hash: String,
    /// The audit payload data
    pub payload: serde_json::Value,
    /// 64-character lowercase hex Ed25519 public key of the signer
    pub signer_public_key: String,
    /// 128-character lowercase hex Ed25519 signature over the signing preimage
    pub signature: String,
    /// 64-character lowercase hex SHA-256 hash of the block preimage
    pub block_hash: String,
}

impl LedgerBlock {
    /// Constructs the signing preimage string:
    /// `"{sequence}:{timestamp}:{prev_hash}:{entry_type:?}:{payload_hash}"`
    pub fn signing_preimage(
        sequence: u64,
        timestamp: &str,
        prev_hash: &str,
        entry_type: &EntryType,
        payload_hash: &str,
    ) -> String {
        format!(
            "{}:{}:{}:{:?}:{}",
            sequence, timestamp, prev_hash, entry_type, payload_hash
        )
    }

    /// Constructs the block preimage string:
    /// `"{sequence}:{timestamp}:{prev_hash}:{entry_type:?}:{payload_hash}:{signer_public_key}:{signature}"`
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
            sequence,
            timestamp,
            prev_hash,
            entry_type,
            payload_hash,
            signer_public_key,
            signature
        )
    }
}

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

/// Serializes an arbitrary `serde_json::Value` into canonical, deterministic bytes.
pub fn canonical_json_bytes(value: &serde_json::Value) -> Result<Vec<u8>, Error> {
    let canonical = canonicalize_json_value(value);
    serde_json::to_vec(&canonical).map_err(|e| Error::SerializationError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_type_parsing_and_display() {
        assert_eq!(
            EntryType::from_str("compliance_audit").unwrap(),
            EntryType::ComplianceAudit
        );
        assert_eq!(
            EntryType::from_str("security_scan").unwrap(),
            EntryType::SecurityScan
        );
        assert_eq!(
            EntryType::from_str("policy_ingest").unwrap(),
            EntryType::PolicyIngest
        );
        assert_eq!(
            EntryType::from_str("codebase_snapshot").unwrap(),
            EntryType::CodebaseSnapshot
        );
        assert_eq!(
            EntryType::from_str("system_event").unwrap(),
            EntryType::SystemEvent
        );

        assert_eq!(
            EntryType::ComplianceAudit.as_str(),
            "compliance_audit"
        );
        assert_eq!(
            format!("{}", EntryType::ComplianceAudit),
            "compliance_audit"
        );
        assert!(EntryType::from_str("unknown_type").is_err());
    }

    #[test]
    fn test_canonical_json_ordering() {
        // Construct JSON object with out-of-order keys
        let v1: serde_json::Value = serde_json::json!({
            "z_key": "last",
            "a_key": "first",
            "nested": {
                "b": 2,
                "a": 1
            }
        });

        let v2: serde_json::Value = serde_json::json!({
            "a_key": "first",
            "nested": {
                "a": 1,
                "b": 2
            },
            "z_key": "last"
        });

        let bytes1 = canonical_json_bytes(&v1).unwrap();
        let bytes2 = canonical_json_bytes(&v2).unwrap();

        assert_eq!(bytes1, bytes2);
        let str1 = String::from_utf8(bytes1).unwrap();
        assert_eq!(str1, r#"{"a_key":"first","nested":{"a":1,"b":2},"z_key":"last"}"#);
    }

    #[test]
    fn test_preimage_construction() {
        let signing = LedgerBlock::signing_preimage(
            0,
            "2026-08-15T00:00:00Z",
            "0000000000000000000000000000000000000000000000000000000000000000",
            &EntryType::ComplianceAudit,
            "abcdef123456",
        );
        assert_eq!(
            signing,
            "0:2026-08-15T00:00:00Z:0000000000000000000000000000000000000000000000000000000000000000:ComplianceAudit:abcdef123456"
        );

        let block = LedgerBlock::block_preimage(
            0,
            "2026-08-15T00:00:00Z",
            "0000000000000000000000000000000000000000000000000000000000000000",
            &EntryType::ComplianceAudit,
            "abcdef123456",
            "pubkey123",
            "sig456",
        );
        assert_eq!(
            block,
            "0:2026-08-15T00:00:00Z:0000000000000000000000000000000000000000000000000000000000000000:ComplianceAudit:abcdef123456:pubkey123:sig456"
        );
    }
}
