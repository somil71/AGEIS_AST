#!/usr/bin/env python3
"""
Generate test fixtures for NEEDLE-SENTINEL E2E Test Suite.
Creates:
- tests/fixtures/policies/
- tests/fixtures/keys/
- tests/fixtures/ledgers/
- tests/fixtures/sample_codebase/
"""

import os
import json
import hashlib
from pathlib import Path
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

BASE_DIR = Path("d:/AEGIS_AST/tests/fixtures")

def ensure_dirs():
    (BASE_DIR / "policies").mkdir(parents=True, exist_ok=True)
    (BASE_DIR / "keys").mkdir(parents=True, exist_ok=True)
    (BASE_DIR / "ledgers").mkdir(parents=True, exist_ok=True)
    (BASE_DIR / "sample_codebase" / "src").mkdir(parents=True, exist_ok=True)

def generate_policies():
    pol_dir = BASE_DIR / "policies"

    # 1. security_standard_v1.md
    sec_v1 = """# Enterprise Security Standard v1.0

## Section 1: Authentication and Credential Storage
### 1.1 Password Hashing Requirement
All user passwords MUST be hashed using bcrypt, argon2, or pbkdf2 prior to persistence. Plaintext password storage is strictly forbidden.

### 1.2 Multi-Factor Authentication
Multi-factor authentication MUST be enforced for all administrative and privileged user accounts.

## Section 2: Cryptographic Controls
### 2.1 Encryption at Rest
All sensitive cardholder and personal identifying data MUST be encrypted at rest using AES-256-GCM or ChaCha20-Poly1305.

### 2.2 Transport Layer Security
All network communications transporting credentials or telemetry MUST use TLS 1.3. Unencrypted HTTP is prohibited.

## Section 3: Audit and Logging
### 3.1 Audit Trail Maintenance
All authentication events, privilege escalations, and cryptographic key generation operations MUST be recorded to an append-only audit ledger.
"""
    (pol_dir / "security_standard_v1.md").write_text(sec_v1, encoding="utf-8")

    # 2. gdpr_data_privacy.txt
    gdpr = """EU General Data Protection Regulation (GDPR) Compliance Standard

Article 5: Principles relating to processing of personal data
Personal data SHALL be processed lawfully, fairly and in a transparent manner. Personal data MUST be collected for specified, explicit and legitimate purposes and not further processed in a manner incompatible with those purposes.

Article 17: Right to erasure (Right to be forgotten)
The data controller SHALL have the obligation to erase personal data without undue delay where the personal data are no longer necessary in relation to the purposes for which they were collected or otherwise processed.

Article 32: Security of processing
Taking into account the state of the art, the costs of implementation and the nature of processing, the controller and processor SHALL implement appropriate technical and organizational measures, including pseudonymization and encryption of personal data.
"""
    (pol_dir / "gdpr_data_privacy.txt").write_text(gdpr, encoding="utf-8")

    # 3. pci_dss_sample.policy
    pci = """PCI DSS Version 4.0 Standard for Payment Card Security

Requirement 3: Protect Stored Account Data
Requirement 3.4: Primary Account Numbers (PAN) MUST be rendered unreadable anywhere it is stored using strong cryptography with associated key management processes.
Requirement 3.5: Cryptographic keys used for encryption of cardholder data MUST be protected against disclosure and unauthorized replacement.

Requirement 8: Identify Users and Authenticate Access
Requirement 8.2: User identification and authentication management systems MUST enforce unique user IDs and strong passwords of at least 12 characters.
Requirement 8.3: Strong multi-factor authentication (MFA) MUST be implemented for all access into the cardholder data environment.
"""
    (pol_dir / "pci_dss_sample.policy").write_text(pci, encoding="utf-8")

    # 4. empty_policy.md (0-byte)
    (pol_dir / "empty_policy.md").write_bytes(b"")

    # 5. whitespace_only.txt
    (pol_dir / "whitespace_only.txt").write_text("   \n\t  \n  \n\r\n", encoding="utf-8")

    # 6. malformed_clauses.md
    malformed = """# [CORRUPTED_HEADER_WITHOUT_CLOSING
§§
Clause ???: Invalid Unicode \x00\x01\x02 test section
- unclosed quote "here
"""
    (pol_dir / "malformed_clauses.md").write_text(malformed, encoding="utf-8", errors="replace")

    # 7. valid_nist_cybersecurity.pdf
    pdf_text = "NIST Cybersecurity Framework Version 2.0 PR.AC Access Control PR.DS Data Security Encryption at Rest PR.IP Information Protection PR.MA Maintenance"
    valid_pdf_bytes = create_simple_pdf(pdf_text)
    (pol_dir / "valid_nist_cybersecurity.pdf").write_bytes(valid_pdf_bytes)

    # 8. scanned_image_only.pdf (0 extractable text characters)
    scanned_pdf_bytes = create_image_only_pdf()
    (pol_dir / "scanned_image_only.pdf").write_bytes(scanned_pdf_bytes)

def create_simple_pdf(text: str) -> bytes:
    stream_content = f"BT /F1 12 Tf 72 712 Td ({text}) Tj ET\n"
    stream_len = len(stream_content.encode("latin1"))

    objects = []
    # 1: Catalog
    objects.append("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")
    # 2: Pages
    objects.append("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n")
    # 3: Page
    objects.append("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n")
    # 4: Contents Stream
    objects.append(f"4 0 obj\n<< /Length {stream_len} >>\nstream\n{stream_content}endstream\nendobj\n")
    # 5: Font
    objects.append("5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n")

    header = "%PDF-1.4\n"
    body = ""
    xref_offsets = [0] # object 0

    current_offset = len(header.encode("latin1"))
    for obj in objects:
        xref_offsets.append(current_offset)
        body += obj
        current_offset += len(obj.encode("latin1"))

    xref_start = current_offset
    xref = f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n"
    for off in xref_offsets[1:]:
        xref += f"{off:010d} 00000 n \n"

    trailer = f"trailer\n<< /Root 1 0 R /Size {len(objects) + 1} >>\nstartxref\n{xref_start}\n%%EOF\n"

    return (header + body + xref + trailer).encode("latin1")

def create_image_only_pdf() -> bytes:
    # A PDF with an image XObject but zero text BT...ET streams
    # Minimal 1x1 8-bit image
    img_data = b"\x80" # single gray pixel
    img_len = len(img_data)

    objects = []
    # 1: Catalog
    objects.append("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")
    # 2: Pages
    objects.append("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n")
    # 3: Page with Image XObject in Resources, no text
    objects.append("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /XObject << /Im1 5 0 R >> >> >>\nendobj\n")
    # 4: Contents (draw image, zero text commands)
    stream_content = "q 612 0 0 792 0 0 cm /Im1 Do Q\n"
    stream_len = len(stream_content.encode("latin1"))
    objects.append(f"4 0 obj\n<< /Length {stream_len} >>\nstream\n{stream_content}endstream\nendobj\n")
    # 5: Image XObject
    objects.append(f"5 0 obj\n<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Length {img_len} >>\nstream\n\x80\nendstream\nendobj\n")

    header = "%PDF-1.4\n"
    body = ""
    xref_offsets = [0]

    current_offset = len(header.encode("latin1"))
    for obj in objects:
        xref_offsets.append(current_offset)
        body += obj
        current_offset += len(obj.encode("latin1"))

    xref_start = current_offset
    xref = f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n"
    for off in xref_offsets[1:]:
        xref += f"{off:010d} 00000 n \n"

    trailer = f"trailer\n<< /Root 1 0 R /Size {len(objects) + 1} >>\nstartxref\n{xref_start}\n%%EOF\n"

    return (header + body + xref + trailer).encode("latin1")

def generate_keys_and_ledgers():
    keys_dir = BASE_DIR / "keys"
    ledgers_dir = BASE_DIR / "ledgers"

    # Deterministic Seed for test_auditor (32 bytes)
    priv_bytes_1 = bytes.fromhex("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
    key1 = Ed25519PrivateKey.from_private_bytes(priv_bytes_1)
    pub_bytes_1 = key1.public_key().public_bytes_raw()
    pub_hex_1 = pub_bytes_1.hex()

    (keys_dir / "test_auditor_ed25519.priv").write_text(priv_bytes_1.hex() + "\n", encoding="utf-8")
    (keys_dir / "test_auditor_ed25519.pub").write_text(pub_hex_1 + "\n", encoding="utf-8")

    # Secondary keypair
    priv_bytes_2 = bytes.fromhex("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb")
    key2 = Ed25519PrivateKey.from_private_bytes(priv_bytes_2)
    pub_bytes_2 = key2.public_key().public_bytes_raw()
    pub_hex_2 = pub_bytes_2.hex()

    (keys_dir / "secondary_auditor.priv").write_text(priv_bytes_2.hex() + "\n", encoding="utf-8")
    (keys_dir / "secondary_auditor.pub").write_text(pub_hex_2 + "\n", encoding="utf-8")

    # Corrupted key file
    (keys_dir / "corrupted_key.priv").write_text("deadbeef00112233\n", encoding="utf-8")

    # -------------------------------------------------------------
    # Generate Ledgers
    # -------------------------------------------------------------

    # Empty chain
    (ledgers_dir / "empty_chain.jsonl").write_bytes(b"")

    def canonical_json(val) -> bytes:
        # sorted keys, no whitespace
        return json.dumps(val, sort_keys=True, separators=(',', ':')).encode('utf-8')

    def sha256_hex(data: bytes) -> str:
        return hashlib.sha256(data).hexdigest()

    GENESIS_PREV_HASH = "0000000000000000000000000000000000000000000000000000000000000000"

    # Build 3 valid blocks
    # Block 0:
    seq0 = 0
    ts0 = "2026-08-15T00:00:00Z"
    prev0 = GENESIS_PREV_HASH
    entry_type_0 = "policy_ingest"
    entry_type_debug_0 = "PolicyIngest"
    payload_0 = {"policy_id": "SEC-V1", "title": "Enterprise Security Standard"}
    p_hash_0 = sha256_hex(canonical_json(payload_0))
    signing_preimage_0 = f"{seq0}:{ts0}:{prev0}:{entry_type_debug_0}:{p_hash_0}"
    sig_0 = key1.sign(signing_preimage_0.encode('utf-8')).hex()
    block_preimage_0 = f"{seq0}:{ts0}:{prev0}:{entry_type_debug_0}:{p_hash_0}:{pub_hex_1}:{sig_0}"
    block_hash_0 = sha256_hex(block_preimage_0.encode('utf-8'))

    block_0 = {
        "sequence": seq0,
        "timestamp": ts0,
        "prev_hash": prev0,
        "entry_type": entry_type_0,
        "payload_hash": p_hash_0,
        "payload": payload_0,
        "signer_public_key": pub_hex_1,
        "signature": sig_0,
        "block_hash": block_hash_0
    }

    # Block 1:
    seq1 = 1
    ts1 = "2026-08-15T00:01:00Z"
    prev1 = block_hash_0
    entry_type_1 = "compliance_audit"
    entry_type_debug_1 = "ComplianceAudit"
    payload_1 = {"score": 100, "status": "compliant", "violations": []}
    p_hash_1 = sha256_hex(canonical_json(payload_1))
    signing_preimage_1 = f"{seq1}:{ts1}:{prev1}:{entry_type_debug_1}:{p_hash_1}"
    sig_1 = key1.sign(signing_preimage_1.encode('utf-8')).hex()
    block_preimage_1 = f"{seq1}:{ts1}:{prev1}:{entry_type_debug_1}:{p_hash_1}:{pub_hex_1}:{sig_1}"
    block_hash_1 = sha256_hex(block_preimage_1.encode('utf-8'))

    block_1 = {
        "sequence": seq1,
        "timestamp": ts1,
        "prev_hash": prev1,
        "entry_type": entry_type_1,
        "payload_hash": p_hash_1,
        "payload": payload_1,
        "signer_public_key": pub_hex_1,
        "signature": sig_1,
        "block_hash": block_hash_1
    }

    # Block 2:
    seq2 = 2
    ts2 = "2026-08-15T00:02:00Z"
    prev2 = block_hash_1
    entry_type_2 = "codebase_snapshot"
    entry_type_debug_2 = "CodebaseSnapshot"
    payload_2 = {"commit": "c0ffee", "files_indexed": 4, "symbols_mapped": 12}
    p_hash_2 = sha256_hex(canonical_json(payload_2))
    signing_preimage_2 = f"{seq2}:{ts2}:{prev2}:{entry_type_debug_2}:{p_hash_2}"
    sig_2 = key1.sign(signing_preimage_2.encode('utf-8')).hex()
    block_preimage_2 = f"{seq2}:{ts2}:{prev2}:{entry_type_debug_2}:{p_hash_2}:{pub_hex_1}:{sig_2}"
    block_hash_2 = sha256_hex(block_preimage_2.encode('utf-8'))

    block_2 = {
        "sequence": seq2,
        "timestamp": ts2,
        "prev_hash": prev2,
        "entry_type": entry_type_2,
        "payload_hash": p_hash_2,
        "payload": payload_2,
        "signer_public_key": pub_hex_1,
        "signature": sig_2,
        "block_hash": block_hash_2
    }

    # 1. valid_three_block_chain.jsonl
    valid_lines = [json.dumps(block_0), json.dumps(block_1), json.dumps(block_2)]
    (ledgers_dir / "valid_three_block_chain.jsonl").write_text("\n".join(valid_lines) + "\n", encoding="utf-8")

    # 2. tampered_payload_seq1.jsonl (modify payload in block 1)
    tampered_b1 = dict(block_1)
    tampered_b1["payload"] = {"score": 42, "status": "tampered", "violations": ["injected"]}
    tampered_p_lines = [json.dumps(block_0), json.dumps(tampered_b1), json.dumps(block_2)]
    (ledgers_dir / "tampered_payload_seq1.jsonl").write_text("\n".join(tampered_p_lines) + "\n", encoding="utf-8")

    # 3. tampered_sequence_gap.jsonl (sequence 1 replaced by sequence 3)
    gap_b1 = dict(block_1)
    gap_b1["sequence"] = 3
    gap_lines = [json.dumps(block_0), json.dumps(gap_b1), json.dumps(block_2)]
    (ledgers_dir / "tampered_sequence_gap.jsonl").write_text("\n".join(gap_lines) + "\n", encoding="utf-8")

    # 4. tampered_prev_hash.jsonl (block 2 prev_hash modified)
    prev_b2 = dict(block_2)
    prev_b2["prev_hash"] = "f" * 64
    prev_lines = [json.dumps(block_0), json.dumps(block_1), json.dumps(prev_b2)]
    (ledgers_dir / "tampered_prev_hash.jsonl").write_text("\n".join(prev_lines) + "\n", encoding="utf-8")

    # 5. tampered_signature.jsonl (block 0 signature corrupted)
    sig_b0 = dict(block_0)
    sig_b0["signature"] = "aa" * 64
    sig_lines = [json.dumps(sig_b0), json.dumps(block_1), json.dumps(block_2)]
    (ledgers_dir / "tampered_signature.jsonl").write_text("\n".join(sig_lines) + "\n", encoding="utf-8")

    # 6. tampered_deleted_block.jsonl (block 1 deleted)
    del_lines = [json.dumps(block_0), json.dumps(block_2)]
    (ledgers_dir / "tampered_deleted_block.jsonl").write_text("\n".join(del_lines) + "\n", encoding="utf-8")

def generate_sample_codebase():
    sc_dir = BASE_DIR / "sample_codebase"
    src_dir = sc_dir / "src"

    cargo_toml = """[package]
name = "sample_codebase"
version = "0.1.0"
edition = "2021"

[dependencies]
"""
    (sc_dir / "Cargo.toml").write_text(cargo_toml, encoding="utf-8")

    lib_rs = """pub mod auth;
pub mod crypto;
pub mod storage;
pub mod network;
"""
    (src_dir / "lib.rs").write_text(lib_rs, encoding="utf-8")

    auth_rs = """//! Authentication, MFA, and Password Hashing

pub fn authenticate_user(username: &str, password_hash: &str) -> bool {
    !username.is_empty() && !password_hash.is_empty()
}

pub fn verify_password_hash(password: &str, hash: &str) -> bool {
    // bcrypt / argon2 password hashing verification
    password.len() >= 8 && hash.starts_with("$argon2id$")
}

pub fn issue_jwt(user_id: &str, secret: &[u8]) -> String {
    format!("jwt_token_for_{}_{}", user_id, secret.len())
}

pub fn enforce_mfa(user_id: &str, totp_code: &str) -> bool {
    totp_code.len() == 6 && !user_id.is_empty()
}
"""
    (src_dir / "auth.rs").write_text(auth_rs, encoding="utf-8")

    crypto_rs = """//! Cryptographic Utilities & AES-256-GCM

pub fn encrypt_aes_gcm(plaintext: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Vec<u8> {
    let mut ciphertext = Vec::with_capacity(plaintext.len() + 16);
    for (i, &b) in plaintext.iter().enumerate() {
        ciphertext.push(b ^ key[i % 32] ^ nonce[i % 12]);
    }
    ciphertext
}

pub fn decrypt_aes_gcm(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Vec<u8> {
    let mut plaintext = Vec::with_capacity(ciphertext.len());
    for (i, &b) in ciphertext.iter().enumerate() {
        plaintext.push(b ^ key[i % 32] ^ nonce[i % 12]);
    }
    plaintext
}

pub fn generate_salt() -> [u8; 16] {
    [0x42; 16]
}
"""
    (src_dir / "crypto.rs").write_text(crypto_rs, encoding="utf-8")

    storage_rs = """//! Data Storage and GDPR Erasure

pub fn store_user_record(user_id: &str, record_data: &[u8]) -> Result<(), String> {
    if user_id.is_empty() {
        return Err("user_id cannot be empty".to_string());
    }
    Ok(())
}

pub fn purge_expired_records(retention_days: u32) -> usize {
    if retention_days > 0 { 10 } else { 0 }
}
"""
    (src_dir / "storage.rs").write_text(storage_rs, encoding="utf-8")

    network_rs = """//! Network and TLS 1.3 Telemetry

pub fn send_telemetry(endpoint: &str, payload: &[u8]) -> Result<(), String> {
    if !endpoint.starts_with("https://") {
        return Err("TLS is required for telemetry".to_string());
    }
    Ok(())
}

pub fn fetch_remote_data(url: &str) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err("Insecure HTTP disallowed".to_string());
    }
    Ok(vec![])
}
"""
    (src_dir / "network.rs").write_text(network_rs, encoding="utf-8")

if __name__ == "__main__":
    ensure_dirs()
    generate_policies()
    generate_keys_and_ledgers()
    generate_sample_codebase()
    print("All fixtures generated successfully under tests/fixtures/!")
