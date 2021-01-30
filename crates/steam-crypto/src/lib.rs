//! Steam System uses RSA to encrypt messages
//! If we want to emulate its behavior we
//! need to encrypt our stuff with the leaked
//! public key
//!
//! Direct Port of
//! https://github.com/DoctorMcKay/node-steam-crypto

#![warn(missing_docs, missing_doc_code_examples)]
#![deny(
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_import_braces,
    unused_qualifications
)]

extern crate crc32fast;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate lazy_static_include;

use bytes::{BufMut, Bytes, BytesMut};
use crc32fast::Hasher;
use openssl::{error::ErrorStack, hash::MessageDigest, rsa::Padding, sign::Verifier};
use rand::prelude::*;

mod symm;
lazy_static_include_bytes!(STEAM_KEY, "assets/steam_public.pem");

#[derive(Debug)]
/// Used by SteamConnection to encrypt messages.
///
/// You should use the one from `encrypted` field.
pub struct SessionKeys {
    plain_text: Vec<u8>,
    /// Generated encryption key after the initial handshake with Steam.
    /// Used to encrypt every message until the end of the Session, where it is discarded.
    pub encrypted: Vec<u8>,
}

pub fn verify_signature(data: &[u8], signature: &[u8]) -> Result<bool, ErrorStack> {
    // standard algorithm is RSA-SHA1
    // but this should be selectable
    let steam_key_bytes: &'static [u8] = *STEAM_KEY;
    let steam_pkey = openssl::pkey::PKey::public_key_from_pem(&steam_key_bytes)?;

    let mut verifier = Verifier::new(MessageDigest::sha1(), &steam_pkey)?;
    verifier.update(&data)?;
    verifier.verify(&signature)
}

/// Returns SessionsKeys struct.
///
/// Generates a 32 byte random blob of data and encrypts it with RSA 1024
/// using the Steam's public key.
///
/// If there is a nonce, it gets concatenated after the generated 32 bytes
pub fn generate_session_key(nonce: Option<&[u8]>) -> Result<SessionKeys, ErrorStack> {
    let mut random_bytes_array = vec![0u8; 32];
    let mut encrypted_array = vec![0u8; 256];

    thread_rng().fill_bytes(&mut random_bytes_array);

    if let Some(nonce) = nonce {
        random_bytes_array.extend(nonce);
    }

    let steam_key: &'static [u8] = *STEAM_KEY;
    let public_key = openssl::rsa::Rsa::public_key_from_pem(&steam_key)?;
    public_key.public_encrypt(&random_bytes_array, &mut encrypted_array, Padding::PKCS1_OAEP)?;

    Ok(SessionKeys {
        plain_text: random_bytes_array,
        encrypted: encrypted_array,
    })
}

/// Performs CRC32 on an input byte array
pub fn crc_hash(input: &[u8]) -> Vec<u8> {
    let mut hasher = Hasher::new();
    hasher.update(&input);

    let checksum = hasher.finalize();
    let mut checksum_bytes: [u8; 4] = checksum.to_be_bytes();
    checksum_bytes.reverse();
    checksum_bytes.to_vec()
}

/// Returns both the `SessionKeys` and a ready to send payload for MsgEncryptRequest
pub fn generate_encrypt_request_handshake(payload: &[u8]) -> (SessionKeys, Bytes) {
    let session_keys = generate_session_key(Some(payload)).unwrap();
    let temp_encrypted_sessionkey = &session_keys.encrypted[..128];

    let mut response = BytesMut::with_capacity(1024);
    let key_hash = crc_hash(temp_encrypted_sessionkey);

    response.put(temp_encrypted_sessionkey);
    response.put(key_hash.as_ref());
    response.put_u32(0);
    (session_keys, response.freeze())
}
