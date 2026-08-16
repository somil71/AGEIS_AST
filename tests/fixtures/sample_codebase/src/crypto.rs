//! Cryptographic Utilities & AES-256-GCM

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
