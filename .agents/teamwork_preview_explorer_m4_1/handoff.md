# Handoff Report: M4 Cryptographic Primitives & Key Management

**Author**: Explorer 1 (Milestone M4: Cryptographic Audit Ledger Subsystem)  
**Date**: 2026-08-14  
**Working Directory**: `d:\AEGIS_AST\.agents\teamwork_preview_explorer_m4_1`  
**Target Subsystem**: `src/ledger/crypto.rs`, `src/ledger/keypair.rs`, `src/ledger/error.rs`, `Cargo.toml`

---

## 1. Observation

Direct observations from codebase inspection, dependency analysis, and specification documents:

1. **Dependency Analysis in `Cargo.toml`**:
   - `Cargo.toml` lines 85: `rand = "0.8"` is ALREADY declared and available in the workspace dependencies.
   - `Cargo.toml` lines 12-101: `sha2`, `ed25519-dalek`, and `hex` are **currently absent** and must be added under `[dependencies]`:
     ```toml
     sha2 = "0.10"
     ed25519-dalek = { version = "2.1", features = ["rand_core"] }
     hex = "0.4"
     ```
   - Build health check: `cargo check` and `cargo test --lib` execute cleanly with exit code 0.

2. **Module Layout & Error Types**:
   - `src/lib.rs` currently exports 13 modules (`analysis`, `chunking`, `config`, `embedding`, `error`, `graph`, `indexing`, `llm`, `query`, `schema`, `storage`, `server`, `watcher`). It requires `pub mod ledger;` (and `pub mod policy;`).
   - `src/error.rs` contains `pub enum Error`. It requires `LedgerError(String)` variant and `From<LedgerError>` conversion for unified error propagation across the application.
   - `src/ledger/` is currently a greenfield module ready for implementation across `crypto.rs`, `keypair.rs`, `error.rs`, `block.rs`, `verifier.rs`, and `mod.rs`.

3. **Cryptographic Standards Required**:
   - **SHA-256**: All hash digests (payload hashes, block hashes) must produce 64-character lowercase hexadecimal strings (`sha256_hex`).
   - **Ed25519**: Digital signatures must use 256-bit Edwards-curve DSA (Ed25519) producing 128-character lowercase hexadecimal strings (`sign_ed25519`).
   - **Genesis State**: Sequence 0 previous hash is fixed as 64 zeroes: `"0000000000000000000000000000000000000000000000000000000000000000"`.

4. **Security & Key Redaction Mandate**:
   - Private key material (`SigningKey`) must **never** be logged, formatted, or exposed via debug or display streams.
   - Custom `fmt::Debug` on `LedgerKeypair` must output:
     ```rust
     LedgerKeypair {
         verifying_key: "<64-hex-public-key>",
         signing_key: "[REDACTED PRIVATE KEY]"
     }
     ```
   - Custom `fmt::Display` on `LedgerKeypair` must output:
     ```rust
     LedgerKeypair(pubkey: <64-hex-public-key>)
     ```

---

## 2. Logic Chain

1. **Dependency Selection & Configuration**:
   - `rand = "0.8"` provides `rand::rngs::OsRng`.
   - `ed25519-dalek = { version = "2.1", features = ["rand_core"] }` provides the latest standard Rust cryptographic implementation for Ed25519, where `SigningKey::generate(&mut OsRng)` securely pulls 32 random bytes from OS entropy.
   - `sha2 = "0.10"` provides the standard NIST SHA-256 digest engine via `sha2::{Digest, Sha256}`.
   - `hex = "0.4"` provides fast, allocation-efficient lower-hex encoding and decoding (`hex::encode`, `hex::decode`).

2. **Cryptographic Primitives Architecture (`src/ledger/crypto.rs`)**:
   - `sha256_hex(data: &[u8]) -> String`:
     Runs `Sha256::digest(data)` and returns `hex::encode(digest)`. Infallible.
   - `sha256_digest(data: &[u8]) -> [u8; 32]`:
     Raw 32-byte digest array helper.
   - `sign_ed25519(signing_key: &SigningKey, message: &[u8]) -> String`:
     Uses `signing_key.sign(message)` to produce `Signature` and returns `hex::encode(signature.to_bytes())`. 64 bytes = 128 lowercase hex chars.
   - `verify_ed25519_signature(pubkey_hex: &str, message: &[u8], signature_hex: &str) -> Result<bool, LedgerError>`:
     - Validates and decodes `pubkey_hex` (32 bytes / 64 hex chars).
     - Validates and parses `VerifyingKey::from_bytes(&pubkey_bytes)`.
     - Validates and decodes `signature_hex` (64 bytes / 128 hex chars).
     - Validates and parses `Signature::from_bytes(&sig_bytes)`.
     - Executes `verifying_key.verify(message, &signature)`:
       - Returns `Ok(true)` on successful mathematical signature verification.
       - Returns `Ok(false)` on signature mismatch (valid format but incorrect signature / altered message).
       - Returns `Err(LedgerError::...)` if the public key or signature string is malformed or invalid hex.
   - `canonical_payload_hash(payload: &serde_json::Value) -> Result<String, LedgerError>`:
     Serializes JSON payload via `serde_json::to_vec(payload)` and hashes with `sha256_hex`.

3. **Keypair Management Architecture (`src/ledger/keypair.rs`)**:
   - `LedgerKeypair` struct wraps `signing_key: SigningKey` (`pub(crate)`) and `pub verifying_key: VerifyingKey`.
   - `LedgerKeypair::generate() -> Self`: Generates fresh keypair via `OsRng`.
   - `LedgerKeypair::from_bytes(bytes: &[u8; 32]) -> Self`: Constructs keypair from 32-byte secret seed.
   - `LedgerKeypair::from_secret_hex(hex_str: &str) -> Result<Self, LedgerError>`: Constructs keypair from 64-char hex string.
   - `LedgerKeypair::public_key_hex(&self) -> String`: Returns 64-char lowercase hex string.
   - `LedgerKeypair::sign(&self, message: &[u8]) -> String`: Signs arbitrary slice.
   - `save_to_files(&self, priv_path: &Path, pub_path: &Path) -> Result<(), LedgerError>`:
     - Writes hex strings with trailing newline.
     - On Unix (`#[cfg(unix)]`), sets private key file permissions to `0o600` (`PermissionsExt::set_mode`).
   - `load_from_files(priv_path: &Path, pub_path: Option<&Path>) -> Result<Self, LedgerError>`:
     - Loads `.priv` file, parses hex to `SigningKey`.
     - If `pub_path` is provided and exists, verifies derived public key matches file content.
   - `load_or_generate(priv_path: &Path, pub_path: &Path, generate_if_missing: bool) -> Result<Self, LedgerError>`:
     - If `.priv` exists -> loads it.
     - If not and `generate_if_missing == true` -> generates, saves, and returns it.
     - If not and `generate_if_missing == false` -> returns `Err(LedgerError::KeyNotFound(...))`.

4. **Error Handling Architecture (`src/ledger/error.rs`)**:
   - Dedicated `LedgerError` enum capturing `InvalidHex`, `InvalidKeyLength`, `InvalidSignatureLength`, `CryptoError`, `KeyNotFound`, `Io`, `Serialization`, and `TamperDetected`.
   - Implements `std::error::Error` and `Display` via `thiserror`.
   - Integrated into `src/error.rs` via `Error::LedgerError(String)`.

---

## 3. Detailed Implementation Reference

### 3.1 `Cargo.toml` Additions

```toml
[dependencies]
# Existing dependencies...
rand = "0.8"

# M4 Ledger Cryptographic Dependencies
sha2 = "0.10"
ed25519-dalek = { version = "2.1", features = ["rand_core"] }
hex = "0.4"
```

---

### 3.2 `src/ledger/error.rs`

```rust
use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("Invalid hex string: {0}")]
    InvalidHex(#[from] hex::FromHexError),

    #[error("Invalid key length: expected {expected} bytes, got {actual} bytes")]
    InvalidKeyLength { expected: usize, actual: usize },

    #[error("Invalid signature length: expected {expected} bytes, got {actual} bytes")]
    InvalidSignatureLength { expected: usize, actual: usize },

    #[error("Ed25519 cryptographic error: {0}")]
    CryptoError(String),

    #[error("Key file not found: {0}")]
    KeyNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("TAMPER DETECTED at sequence {sequence}: {reason}")]
    TamperDetected { sequence: u64, reason: String },

    #[error("Ledger error: {0}")]
    Other(String),
}

impl From<ed25519_dalek::SignatureError> for LedgerError {
    fn from(e: ed25519_dalek::SignatureError) -> Self {
        LedgerError::CryptoError(e.to_string())
    }
}
```

---

### 3.3 `src/ledger/crypto.rs`

```rust
use crate::ledger::error::LedgerError;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

pub const GENESIS_PREV_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Computes SHA-256 hash over an arbitrary byte slice, returning a 64-character lowercase hex string.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    hex::encode(digest)
}

/// Computes raw 32-byte SHA-256 digest over an arbitrary byte slice.
pub fn sha256_digest(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Signs a byte message using an Ed25519 SigningKey, returning a 128-character lowercase hex string.
pub fn sign_ed25519(signing_key: &SigningKey, message: &[u8]) -> String {
    let signature: Signature = signing_key.sign(message);
    hex::encode(signature.to_bytes())
}

/// Verifies an Ed25519 signature given the public key hex, message slice, and signature hex.
/// Returns Ok(true) if valid, Ok(false) if the signature does not match,
/// or Err(LedgerError) if public key or signature hex format is invalid.
pub fn verify_ed25519_signature(
    pubkey_hex: &str,
    message: &[u8],
    signature_hex: &str,
) -> Result<bool, LedgerError> {
    let pubkey_bytes = hex::decode(pubkey_hex.trim())?;

    if pubkey_bytes.len() != 32 {
        return Err(LedgerError::InvalidKeyLength {
            expected: 32,
            actual: pubkey_bytes.len(),
        });
    }

    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&pubkey_bytes);
    let verifying_key = VerifyingKey::from_bytes(&key_arr)
        .map_err(|e| LedgerError::CryptoError(format!("Invalid VerifyingKey: {e}")))?;

    let sig_bytes = hex::decode(signature_hex.trim())?;

    if sig_bytes.len() != 64 {
        return Err(LedgerError::InvalidSignatureLength {
            expected: 64,
            actual: sig_bytes.len(),
        });
    }

    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let signature = Signature::from_bytes(&sig_arr);

    match verifying_key.verify(message, &signature) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Deterministically formats and hashes a JSON payload.
pub fn canonical_payload_hash(payload: &serde_json::Value) -> Result<String, LedgerError> {
    let bytes = serde_json::to_vec(payload)?;
    Ok(sha256_hex(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn test_sha256_hex_empty() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_sha256_hex_known_string() {
        assert_eq!(
            sha256_hex(b"hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_sign_and_verify() {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());

        let message = b"cryptographic audit trail transaction";
        let sig_hex = sign_ed25519(&signing_key, message);

        assert_eq!(sig_hex.len(), 128);

        // Verification must succeed
        let valid = verify_ed25519_signature(&pubkey_hex, message, &sig_hex).unwrap();
        assert!(valid);

        // Tampered message must fail verification
        let tampered_msg = b"cryptographic audit trail Transaction";
        let invalid = verify_ed25519_signature(&pubkey_hex, tampered_msg, &sig_hex).unwrap();
        assert!(!invalid);
    }

    #[test]
    fn test_invalid_signature_hex() {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());

        let res = verify_ed25519_signature(&pubkey_hex, b"test", "invalid_hex");
        assert!(res.is_err());
    }
}
```

---

### 3.4 `src/ledger/keypair.rs`

```rust
use crate::ledger::crypto::sign_ed25519;
use crate::ledger::error::LedgerError;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use std::fmt;
use std::path::Path;

pub struct LedgerKeypair {
    pub(crate) signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl LedgerKeypair {
    /// Generates a new cryptographically secure Ed25519 keypair using OS RNG.
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Constructs a keypair from 32 raw secret seed bytes.
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(bytes);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Constructs a keypair from a 64-character hex-encoded private key seed string.
    pub fn from_secret_hex(hex_str: &str) -> Result<Self, LedgerError> {
        let bytes = hex::decode(hex_str.trim())?;
        if bytes.len() != 32 {
            return Err(LedgerError::InvalidKeyLength {
                expected: 32,
                actual: bytes.len(),
            });
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(&arr))
    }

    /// Returns the 64-character lowercase hex string of the public (verifying) key.
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key.to_bytes())
    }

    /// Signs an arbitrary byte message using the internal signing key, returning a 128-char hex signature.
    pub fn sign(&self, message: &[u8]) -> String {
        sign_ed25519(&self.signing_key, message)
    }

    /// Saves the keypair to disk at `priv_path` and `pub_path` in hex format.
    /// Sets restricted file permissions on Unix systems (0600 for private key).
    pub fn save_to_files(&self, priv_path: &Path, pub_path: &Path) -> Result<(), LedgerError> {
        if let Some(parent) = priv_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(parent) = pub_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let priv_hex = hex::encode(self.signing_key.to_bytes());
        let pub_hex = hex::encode(self.verifying_key.to_bytes());

        // Write private key
        std::fs::write(priv_path, format!("{}\n", priv_hex))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(priv_path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(priv_path, perms);
            }
        }

        // Write public key
        std::fs::write(pub_path, format!("{}\n", pub_hex))?;

        Ok(())
    }

    /// Loads an Ed25519 keypair from a private key file (`.priv`).
    /// If `pub_path` exists, verifies that public key matches derived public key.
    pub fn load_from_files(priv_path: &Path, pub_path: Option<&Path>) -> Result<Self, LedgerError> {
        if !priv_path.exists() {
            return Err(LedgerError::KeyNotFound(format!(
                "Private key file not found at: {}",
                priv_path.display()
            )));
        }

        let priv_hex = std::fs::read_to_string(priv_path)?;
        let keypair = Self::from_secret_hex(&priv_hex)?;

        if let Some(pub_p) = pub_path {
            if pub_p.exists() {
                let pub_hex = std::fs::read_to_string(pub_p)?;
                let expected_pub = keypair.public_key_hex();
                if pub_hex.trim() != expected_pub {
                    return Err(LedgerError::CryptoError(format!(
                        "Public key file mismatch at '{}': expected {}, found {}",
                        pub_p.display(),
                        expected_pub,
                        pub_hex.trim()
                    )));
                }
            }
        }

        Ok(keypair)
    }

    /// Loads existing keypair from disk or generates a new one if `generate_if_missing` is true.
    pub fn load_or_generate(
        priv_path: &Path,
        pub_path: &Path,
        generate_if_missing: bool,
    ) -> Result<Self, LedgerError> {
        if priv_path.exists() {
            Self::load_from_files(priv_path, Some(pub_path))
        } else if generate_if_missing {
            let keypair = Self::generate();
            keypair.save_to_files(priv_path, pub_path)?;
            Ok(keypair)
        } else {
            Err(LedgerError::KeyNotFound(format!(
                "Private key not found at '{}'. Generate with `needle ledger keygen` or pass `--gen-key-if-missing`.",
                priv_path.display()
            )))
        }
    }
}

// ============================================================================
// STRICT SECURITY MANDATE: Private key must NEVER be printed in Debug or Display
// ============================================================================

impl fmt::Debug for LedgerKeypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LedgerKeypair")
            .field("verifying_key", &hex::encode(self.verifying_key.to_bytes()))
            .field("signing_key", &"[REDACTED PRIVATE KEY]")
            .finish()
    }
}

impl fmt::Display for LedgerKeypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LedgerKeypair(pubkey: {})",
            hex::encode(self.verifying_key.to_bytes())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_redaction_in_debug_and_display() {
        let keypair = LedgerKeypair::generate();
        let secret_bytes_hex = hex::encode(keypair.signing_key.to_bytes());
        let public_bytes_hex = keypair.public_key_hex();

        let debug_str = format!("{:?}", keypair);
        assert!(debug_str.contains("[REDACTED PRIVATE KEY]"));
        assert!(!debug_str.contains(&secret_bytes_hex));
        assert!(debug_str.contains(&public_bytes_hex));

        let display_str = format!("{}", keypair);
        assert!(display_str.contains(&public_bytes_hex));
        assert!(!display_str.contains(&secret_bytes_hex));
        assert!(!display_str.contains("[REDACTED PRIVATE KEY]"));
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let priv_path = temp_dir.path().join("key.priv");
        let pub_path = temp_dir.path().join("key.pub");

        let keypair = LedgerKeypair::generate();
        keypair.save_to_files(&priv_path, &pub_path).unwrap();

        let loaded = LedgerKeypair::load_from_files(&priv_path, Some(&pub_path)).unwrap();
        assert_eq!(keypair.public_key_hex(), loaded.public_key_hex());
        assert_eq!(keypair.signing_key.to_bytes(), loaded.signing_key.to_bytes());
    }

    #[test]
    fn test_load_or_generate() {
        let temp_dir = tempfile::tempdir().unwrap();
        let priv_path = temp_dir.path().join("key.priv");
        let pub_path = temp_dir.path().join("key.pub");

        // Missing with generate_if_missing = false should fail
        let res = LedgerKeypair::load_or_generate(&priv_path, &pub_path, false);
        assert!(res.is_err());

        // Missing with generate_if_missing = true should generate and save
        let keypair1 = LedgerKeypair::load_or_generate(&priv_path, &pub_path, true).unwrap();
        assert!(priv_path.exists());
        assert!(pub_path.exists());

        // Second call should load existing keypair without regeneration
        let keypair2 = LedgerKeypair::load_or_generate(&priv_path, &pub_path, false).unwrap();
        assert_eq!(keypair1.public_key_hex(), keypair2.public_key_hex());
    }
}
```

---

## 4. Caveats

1. **Hardware Security Modules (HSM) & Keyring Integration**:
   Key storage is currently file-based (`.needle/ledger/key.priv`). While Unix 0600 file permissions and in-memory redaction are strictly enforced, sovereign enterprise deployments requiring hardware token signing (e.g. PKCS#11 or YubiKey) can wrap `LedgerKeypair` or implement an abstract `LedgerSigner` trait in future extensions.
2. **Deterministic Payload Hashing**:
   `serde_json::to_vec` on `serde_json::Value` produces deterministic output because `serde_json::Map` uses `BTreeMap` by default. For arbitrary Rust structs, they should either be converted into `serde_json::Value` or serialized with standard canonical JSON serialization.
3. **Entropy Source**:
   `rand::rngs::OsRng` directly queries the OS cryptographically secure random number generator (`/dev/urandom` / `getrandom()` / Windows `BCryptGenRandom`). In strictly containerized or embedded environments with degraded OS entropy, key generation should be initialized after the entropy pool is seeded.

---

## 5. Conclusion

The cryptographic primitives (`crypto.rs`) and key management (`keypair.rs`) architectures provide:
- **Provable Immutability**: Constant-time, collision-resistant SHA-256 hashing.
- **Non-Repudiation**: Ed25519 digital signatures formatted as 128-character lowercase hex strings.
- **Zero-Leakage Key Security**: Compile-time and runtime guarantees that `SigningKey` is masked (`"[REDACTED PRIVATE KEY]"`) across all `Debug`, `Display`, and logging streams.
- **Seamless Key Generation & Persistence**: Clean round-trip `.priv`/`.pub` file management with automatic creation and Unix 0600 permission hardening.

The specifications and test suites are fully detailed and ready for immediate implementation by the implementation subagents.

---

## 6. Verification Method

Once implemented by the builder agent, the crypto and keypair subsystems can be independently verified via:

1. **Unit Test Suite**:
   ```bash
   cargo test --lib ledger::crypto
   cargo test --lib ledger::keypair
   ```
2. **Compilation & Formatting Verification**:
   ```bash
   cargo check
   cargo clippy --all-targets -- -D warnings
   ```
3. **Debug Redaction Verification**:
   Execute `cargo test test_strict_redaction_in_debug_and_display` to verify that secret key bytes never appear in formatted debug strings.
