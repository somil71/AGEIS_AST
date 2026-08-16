//! Ed25519 keypair management, secure persistence, and strict redaction.

use crate::ledger::crypto::sign_ed25519;
use crate::Error;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use std::fmt;
use std::path::Path;

/// Managed Ed25519 keypair for cryptographic audit ledger signing and verification.
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
    pub fn from_secret_hex(hex_str: &str) -> Result<Self, Error> {
        let bytes = hex::decode(hex_str.trim())?;
        if bytes.len() != 32 {
            return Err(Error::LedgerError(format!(
                "Invalid private key length: expected 32 bytes (64 hex characters), got {} bytes",
                bytes.len()
            )));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(&arr))
    }

    /// Returns the 64-character lowercase hex string of the public (verifying) key.
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key.to_bytes())
    }

    /// Signs an arbitrary byte message using the internal signing key, returning a 128-char lowercase hex signature.
    pub fn sign(&self, message: &[u8]) -> String {
        sign_ed25519(&self.signing_key, message)
    }

    /// Saves the keypair to disk at `priv_path` and `pub_path` in hex format.
    /// On Unix systems, sets 0600 permissions on the private key file.
    pub fn save_to_files(&self, priv_path: &Path, pub_path: &Path) -> Result<(), Error> {
        if let Some(parent) = priv_path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        if let Some(parent) = pub_path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        let priv_hex = hex::encode(self.signing_key.to_bytes());
        let pub_hex = hex::encode(self.verifying_key.to_bytes());

        // Write private key
        std::fs::write(priv_path, format!("{}\n", priv_hex)).map_err(Error::Io)?;

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
        std::fs::write(pub_path, format!("{}\n", pub_hex)).map_err(Error::Io)?;

        Ok(())
    }

    /// Loads an Ed25519 keypair from a private key file (`.priv`).
    pub fn load_from_file(priv_path: &Path) -> Result<Self, Error> {
        Self::load_from_files(priv_path, None)
    }

    /// Loads an Ed25519 keypair from a private key file (`.priv`).
    /// If `pub_path` is provided and exists, verifies that public key matches the derived public key.
    pub fn load_from_files(priv_path: &Path, pub_path: Option<&Path>) -> Result<Self, Error> {
        if !priv_path.exists() {
            return Err(Error::LedgerError(format!(
                "Private key file not found at: {}",
                priv_path.display()
            )));
        }

        let priv_hex = std::fs::read_to_string(priv_path).map_err(Error::Io)?;
        let keypair = Self::from_secret_hex(&priv_hex)?;

        if let Some(pub_p) = pub_path {
            if pub_p.exists() {
                let pub_hex = std::fs::read_to_string(pub_p).map_err(Error::Io)?;
                let expected_pub = keypair.public_key_hex();
                if pub_hex.trim() != expected_pub {
                    return Err(Error::LedgerError(format!(
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

    /// Loads an existing keypair from disk or generates a new one if `generate_if_missing` is true.
    pub fn load_or_generate(
        priv_path: &Path,
        pub_path: &Path,
        generate_if_missing: bool,
    ) -> Result<Self, Error> {
        if priv_path.exists() {
            Self::load_from_files(priv_path, Some(pub_path))
        } else if generate_if_missing {
            let keypair = Self::generate();
            keypair.save_to_files(priv_path, pub_path)?;
            Ok(keypair)
        } else {
            Err(Error::LedgerError(format!(
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
            .field("verifying_key", &self.public_key_hex())
            .field("signing_key", &"[REDACTED PRIVATE KEY]")
            .finish()
    }
}

impl fmt::Display for LedgerKeypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LedgerKeypair(pubkey: {})", self.public_key_hex())
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
        assert_eq!(
            keypair.signing_key.to_bytes(),
            loaded.signing_key.to_bytes()
        );
    }

    #[test]
    fn test_load_from_single_priv_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let priv_path = temp_dir.path().join("key.priv");
        let pub_path = temp_dir.path().join("key.pub");

        let keypair = LedgerKeypair::generate();
        keypair.save_to_files(&priv_path, &pub_path).unwrap();

        let loaded = LedgerKeypair::load_from_file(&priv_path).unwrap();
        assert_eq!(keypair.public_key_hex(), loaded.public_key_hex());
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
