//! Cryptographic hashing (SHA-256) and Ed25519 digital signature primitives.

use crate::Error;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

/// Genesis previous hash: 64 zeroes.
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
///
/// Returns `Ok(true)` if valid, `Ok(false)` if the signature format is valid but the signature
/// does not match the message, or `Err(Error)` if public key or signature hex format is invalid.
pub fn verify_ed25519_signature(
    pubkey_hex: &str,
    message: &[u8],
    signature_hex: &str,
) -> Result<bool, Error> {
    let pubkey_bytes = hex::decode(pubkey_hex.trim())?;
    if pubkey_bytes.len() != 32 {
        return Err(Error::LedgerError(format!(
            "Invalid public key length: expected 32 bytes (64 hex characters), got {} bytes",
            pubkey_bytes.len()
        )));
    }

    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&pubkey_bytes);
    let verifying_key = VerifyingKey::from_bytes(&key_arr)
        .map_err(|e| Error::LedgerError(format!("Invalid VerifyingKey: {e}")))?;

    let sig_bytes = hex::decode(signature_hex.trim())?;
    if sig_bytes.len() != 64 {
        return Err(Error::LedgerError(format!(
            "Invalid signature length: expected 64 bytes (128 hex characters), got {} bytes",
            sig_bytes.len()
        )));
    }

    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let signature = Signature::from_bytes(&sig_arr);

    match verifying_key.verify(message, &signature) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
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
    fn test_sha256_digest_matches_hex() {
        let data = b"deterministic cryptographic payload";
        let raw = sha256_digest(data);
        let hex_str = sha256_hex(data);
        assert_eq!(hex::encode(raw), hex_str);
    }

    #[test]
    fn test_sign_and_verify_ed25519() {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());

        let message = b"cryptographic audit trail transaction";
        let sig_hex = sign_ed25519(&signing_key, message);

        assert_eq!(sig_hex.len(), 128);

        // Verification succeeds
        let valid = verify_ed25519_signature(&pubkey_hex, message, &sig_hex).unwrap();
        assert!(valid);

        // Tampered message fails verification (returns Ok(false))
        let tampered_msg = b"cryptographic audit trail Transaction";
        let invalid = verify_ed25519_signature(&pubkey_hex, tampered_msg, &sig_hex).unwrap();
        assert!(!invalid);

        // Wrong public key fails verification
        let other_key = SigningKey::generate(&mut csprng);
        let other_pubkey_hex = hex::encode(other_key.verifying_key().to_bytes());
        let wrong_key_valid =
            verify_ed25519_signature(&other_pubkey_hex, message, &sig_hex).unwrap();
        assert!(!wrong_key_valid);
    }

    #[test]
    fn test_invalid_hex_and_lengths() {
        let res_bad_hex = verify_ed25519_signature("not_hex", b"test", "deadbeef");
        assert!(res_bad_hex.is_err());

        // Valid hex but wrong length
        let res_short_key = verify_ed25519_signature("abcd", b"test", &"aa".repeat(64));
        assert!(res_short_key.is_err());

        let res_short_sig = verify_ed25519_signature(&"aa".repeat(32), b"test", "abcd");
        assert!(res_short_sig.is_err());
    }
}
