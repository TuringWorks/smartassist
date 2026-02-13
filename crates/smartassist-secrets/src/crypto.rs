//! AES-256-GCM encryption with HKDF-SHA256 key derivation.
//!
//! Each secret gets a unique random salt; the master key is never used
//! directly as a cipher key. A fresh random nonce is prepended to the
//! ciphertext so callers only need to keep track of (ciphertext, salt).

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;

use crate::error::{Result, SecretError};

const NONCE_SIZE: usize = 12;
const SALT_SIZE: usize = 32;
const KEY_SIZE: usize = 32;

/// HKDF info string used to domain-separate derived keys.
const HKDF_INFO: &[u8] = b"smartassist-secret-v1";

/// Derive a 256-bit encryption key from `master_key` and `salt` via HKDF-SHA256.
fn derive_key(master_key: &[u8], salt: &[u8]) -> [u8; KEY_SIZE] {
    let hk = Hkdf::<Sha256>::new(Some(salt), master_key);
    let mut okm = [0u8; KEY_SIZE];
    // expand cannot fail when output length <= 255 * hash-length
    hk.expand(HKDF_INFO, &mut okm)
        .expect("HKDF expand should not fail for 32-byte output");
    okm
}

/// Encrypt `plaintext` using a key derived from `master_key`.
///
/// Returns `(nonce || ciphertext_with_tag, salt)`. The salt is randomly
/// generated so the same plaintext encrypted twice produces different output.
pub fn encrypt(master_key: &[u8], plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut salt = vec![0u8; SALT_SIZE];
    rand::thread_rng().fill_bytes(&mut salt);

    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let key = derive_key(master_key, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| SecretError::EncryptionFailed(e.to_string()))?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| SecretError::EncryptionFailed(e.to_string()))?;

    // Prepend nonce to ciphertext so decrypt can split it back out.
    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok((result, salt))
}

/// Decrypt data previously produced by [`encrypt`].
///
/// `encrypted` must contain the nonce followed by the AES-GCM ciphertext
/// (including the authentication tag). `salt` is the same salt returned by
/// the corresponding encrypt call.
pub fn decrypt(master_key: &[u8], encrypted: &[u8], salt: &[u8]) -> Result<Vec<u8>> {
    if encrypted.len() < NONCE_SIZE {
        return Err(SecretError::DecryptionFailed(
            "ciphertext too short".to_string(),
        ));
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_SIZE);

    let key = derive_key(master_key, salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| SecretError::DecryptionFailed(e.to_string()))?;

    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| SecretError::DecryptionFailed(e.to_string()))
}

/// Generate a new random 256-bit master key.
pub fn generate_master_key() -> Vec<u8> {
    let mut key = vec![0u8; KEY_SIZE];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip_encrypt_decrypt() {
        let master_key = generate_master_key();
        let plaintext = b"hello, secret world!";

        let (encrypted, salt) = encrypt(&master_key, plaintext).unwrap();
        let decrypted = decrypt(&master_key, &encrypted, &salt).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key_a = generate_master_key();
        let key_b = generate_master_key();
        let plaintext = b"sensitive data";

        let (encrypted, salt) = encrypt(&key_a, plaintext).unwrap();
        let result = decrypt(&key_b, &encrypted, &salt);

        assert!(result.is_err(), "decryption with wrong key should fail");
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let master_key = generate_master_key();
        let plaintext = b"important secret";

        let (mut encrypted, salt) = encrypt(&master_key, plaintext).unwrap();

        // Flip a byte in the ciphertext portion (after the nonce).
        let idx = NONCE_SIZE + 1;
        encrypted[idx] ^= 0xff;

        let result = decrypt(&master_key, &encrypted, &salt);
        assert!(
            result.is_err(),
            "tampered ciphertext should fail authentication"
        );
    }

    #[test]
    fn test_different_salts_produce_different_ciphertexts() {
        let master_key = generate_master_key();
        let plaintext = b"same plaintext";

        let (enc_a, salt_a) = encrypt(&master_key, plaintext).unwrap();
        let (enc_b, salt_b) = encrypt(&master_key, plaintext).unwrap();

        // Different salts (and nonces) should produce different ciphertext.
        assert_ne!(salt_a, salt_b);
        assert_ne!(enc_a, enc_b);
    }

    #[test]
    fn test_empty_plaintext_works() {
        let master_key = generate_master_key();
        let plaintext = b"";

        let (encrypted, salt) = encrypt(&master_key, plaintext).unwrap();
        let decrypted = decrypt(&master_key, &encrypted, &salt).unwrap();

        assert_eq!(decrypted, plaintext);
    }
}
