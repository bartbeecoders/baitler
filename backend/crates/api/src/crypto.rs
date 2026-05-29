//! Authenticated symmetric encryption for secrets at rest (e.g. provider API
//! keys). ChaCha20-Poly1305 with a 32-byte key derived from the app secret.
//!
//! Ciphertext is stored as hex of `nonce (12 bytes) || ciphertext+tag`.

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("encryption failed")]
    Encrypt,
    #[error("decryption failed")]
    Decrypt,
    #[error("malformed ciphertext")]
    Malformed,
}

/// Derive a 32-byte key from an arbitrary-length app secret.
pub fn derive_key(secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"baitler:secrets:v1");
    hasher.update(secret.as_bytes());
    hasher.finalize().into()
}

/// Encrypt `plaintext`, returning hex of `nonce || ciphertext`.
pub fn encrypt(key: &[u8; 32], plaintext: &str) -> Result<String, CryptoError> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| CryptoError::Encrypt)?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(hex::encode(out))
}

/// Decrypt a hex string produced by [`encrypt`].
pub fn decrypt(key: &[u8; 32], hexed: &str) -> Result<String, CryptoError> {
    let bytes = hex::decode(hexed).map_err(|_| CryptoError::Malformed)?;
    if bytes.len() < 12 {
        return Err(CryptoError::Malformed);
    }
    let (nonce, ciphertext) = bytes.split_at(12);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| CryptoError::Decrypt)?;
    String::from_utf8(plaintext).map_err(|_| CryptoError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let key = derive_key("test-secret");
        let ct = encrypt(&key, "sk-secret-value").unwrap();
        assert_ne!(ct, "sk-secret-value");
        assert_eq!(decrypt(&key, &ct).unwrap(), "sk-secret-value");
    }

    #[test]
    fn different_nonces_produce_different_ciphertexts() {
        let key = derive_key("k");
        assert_ne!(encrypt(&key, "x").unwrap(), encrypt(&key, "x").unwrap());
    }

    #[test]
    fn wrong_key_fails() {
        let ct = encrypt(&derive_key("a"), "secret").unwrap();
        assert!(decrypt(&derive_key("b"), &ct).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = derive_key("k");
        let mut ct = encrypt(&key, "secret").unwrap();
        // Flip the last hex nibble.
        let last = ct.pop().unwrap();
        ct.push(if last == 'a' { 'b' } else { 'a' });
        assert!(decrypt(&key, &ct).is_err());
    }
}
