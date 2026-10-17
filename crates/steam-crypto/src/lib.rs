//! Cryptography used by the Steam CM protocol.
//!
//! When a client connects, Steam sends a `ChannelEncryptRequest` with a random nonce. The client generates a 32-byte
//! session key, encrypts it (followed by the nonce) with the Steam universe's RSA public key, and sends it back on
//! `ChannelEncryptResponse`. From then on every packet is encrypted with AES-256 using that session key, with an IV
//! that carries an HMAC of the message (see [`symmetric_encrypt_hmac_iv`]).
//!
//! Port of SteamKit's [`CryptoHelper`](https://github.com/SteamRE/SteamKit/blob/master/SteamKit2/SteamKit2/Util/CryptoHelper.cs)
//! and [node-steam-crypto](https://github.com/DoctorMcKay/node-steam-crypto).

#![warn(missing_docs)]
#![deny(
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_import_braces,
    unused_qualifications
)]

use std::fmt;
use std::sync::OnceLock;

use rand::thread_rng;
use rand::RngCore;
use rsa::pkcs8::DecodePublicKey;
use rsa::Oaep;
use rsa::Pkcs1v15Sign;
use rsa::RsaPublicKey;
use sha1::Digest;
use sha1::Sha1;
pub use symm::symmetric_decrypt;
pub use symm::symmetric_decrypt_hmac_iv;
pub use symm::symmetric_encrypt;
pub use symm::symmetric_encrypt_hmac_iv;
pub use symm::symmetric_encrypt_with_iv;

mod symm;

/// Public key of the Steam Public universe.
const STEAM_PUBLIC_KEY_PEM: &str = include_str!("../assets/steam_public.pem");

/// Size in bytes of the AES-256 session key.
pub const SESSION_KEY_SIZE: usize = 32;

/// Errors returned by this crate.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CryptoError {
    /// The RSA encryption of the session key failed.
    #[error("Failed to encrypt the session key with RSA: {0}")]
    Rsa(#[from] rsa::Error),

    /// A ciphertext must hold the encrypted IV followed by at least one whole AES block.
    #[error("Ciphertext of {0} bytes is not an encrypted IV followed by whole AES blocks.")]
    InvalidLength(usize),

    /// The decrypted message did not end with valid PKCS#7 padding: wrong key, or corrupted data.
    #[error("Decrypted message has invalid padding.")]
    InvalidPadding,

    /// The HMAC carried in the IV does not match the message: wrong key, or tampered data.
    #[error("Message HMAC does not match.")]
    InvalidHmac,
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, CryptoError>;

/// AES-256 key that encrypts a CM session, after the channel encryption handshake.
///
/// Its `Debug` output does not include the key.
// Not `Copy`, so the key is not duplicated by accident.
#[allow(missing_copy_implementations)]
#[derive(Clone, PartialEq, Eq)]
pub struct SessionKey([u8; SESSION_KEY_SIZE]);

impl SessionKey {
    /// Generates a random session key.
    pub fn generate() -> Self {
        let mut key = [0u8; SESSION_KEY_SIZE];
        thread_rng().fill_bytes(&mut key);
        Self(key)
    }

    /// The raw key bytes.
    pub fn as_bytes(&self) -> &[u8; SESSION_KEY_SIZE] {
        &self.0
    }
}

impl From<[u8; SESSION_KEY_SIZE]> for SessionKey {
    fn from(key: [u8; SESSION_KEY_SIZE]) -> Self {
        Self(key)
    }
}

impl fmt::Debug for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionKey(..)")
    }
}

fn steam_public_key() -> &'static RsaPublicKey {
    static KEY: OnceLock<RsaPublicKey> = OnceLock::new();
    KEY.get_or_init(|| {
        RsaPublicKey::from_public_key_pem(STEAM_PUBLIC_KEY_PEM).expect("bundled Steam public key is valid")
    })
}

/// Verifies an RSA PKCS#1 v1.5 SHA-1 signature made by Steam over `data`.
pub fn verify_signature(data: &[u8], signature: &[u8]) -> bool {
    let digest = Sha1::digest(data);
    steam_public_key()
        .verify(Pkcs1v15Sign::new::<Sha1>(), &digest, signature)
        .is_ok()
}

/// Generates a random session key, and encrypts it followed by `nonce` with the Steam public key (RSA OAEP SHA-1).
///
/// Returns the key and the encrypted blob (128 bytes, the size of the RSA-1024 modulus).
pub fn generate_session_key(nonce: &[u8]) -> Result<(SessionKey, Vec<u8>)> {
    let key = SessionKey::generate();

    let mut blob = Vec::with_capacity(SESSION_KEY_SIZE + nonce.len());
    blob.extend_from_slice(key.as_bytes());
    blob.extend_from_slice(nonce);

    let encrypted = steam_public_key().encrypt(&mut thread_rng(), Oaep::new::<Sha1>(), &blob)?;
    Ok((key, encrypted))
}

/// CRC32 of `input`, in little endian.
pub fn crc_hash(input: &[u8]) -> [u8; 4] {
    crc32fast::hash(input).to_le_bytes()
}

/// Answers a `ChannelEncryptRequest` carrying `nonce`.
///
/// Returns the session key, and the payload that goes after the `MsgChannelEncryptResponse` body: the encrypted
/// session key, its CRC32, and four zero bytes.
pub fn generate_encrypt_request_handshake(nonce: &[u8]) -> Result<(SessionKey, Vec<u8>)> {
    let (key, encrypted_key) = generate_session_key(nonce)?;

    let mut payload = Vec::with_capacity(encrypted_key.len() + 8);
    payload.extend_from_slice(&encrypted_key);
    payload.extend_from_slice(&crc_hash(&encrypted_key));
    payload.extend_from_slice(&[0; 4]);
    Ok((key, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_payload_layout() {
        let nonce = [7u8; 16];
        let (_, payload) = generate_encrypt_request_handshake(&nonce).unwrap();

        assert_eq!(payload.len(), 128 + 4 + 4);
        let (encrypted_key, rest) = payload.split_at(128);
        assert_eq!(&rest[..4], &crc_hash(encrypted_key));
        assert_eq!(&rest[4..], &[0; 4]);
    }

    #[test]
    fn session_keys_are_random() {
        let (first, first_blob) = generate_session_key(&[]).unwrap();
        let (second, second_blob) = generate_session_key(&[]).unwrap();
        assert_ne!(first, second);
        assert_ne!(first_blob, second_blob);
    }

    #[test]
    fn rejects_nonce_too_long_for_rsa() {
        // RSA-1024 OAEP SHA-1 fits 86 bytes: the 32-byte key leaves room for a 54-byte nonce.
        assert!(generate_session_key(&[0; 54]).is_ok());

        let expected = Err(CryptoError::Rsa(rsa::Error::MessageTooLong));
        assert_eq!(generate_session_key(&[0; 55]), expected);
        assert_eq!(generate_encrypt_request_handshake(&[0; 55]), expected);
    }

    #[test]
    fn error_messages() {
        assert_eq!(
            CryptoError::Rsa(rsa::Error::MessageTooLong).to_string(),
            "Failed to encrypt the session key with RSA: message too long"
        );
        assert_eq!(
            CryptoError::InvalidLength(15).to_string(),
            "Ciphertext of 15 bytes is not an encrypted IV followed by whole AES blocks."
        );
        assert_eq!(
            CryptoError::InvalidPadding.to_string(),
            "Decrypted message has invalid padding."
        );
        assert_eq!(CryptoError::InvalidHmac.to_string(), "Message HMAC does not match.");
    }

    #[test]
    fn crc_is_little_endian() {
        // CRC32("123456789") = 0xCBF43926
        assert_eq!(crc_hash(b"123456789"), [0x26, 0x39, 0xF4, 0xCB]);
    }

    #[test]
    fn rejects_bad_signature() {
        assert!(!verify_signature(b"data", &[0u8; 128]));
        assert!(!verify_signature(b"data", &[]));
    }

    #[test]
    fn debug_hides_session_key() {
        let key = SessionKey::from([0xAB; SESSION_KEY_SIZE]);
        assert_eq!(format!("{:?}", key), "SessionKey(..)");
    }
}
