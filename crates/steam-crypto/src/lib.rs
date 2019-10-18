//! Steam System uses RSA to encrypt messages
//! If we want to emulate its behavior we
//! need to encrypt our stuff with the leaked
//! public key
//!
//! Direct Port of
//! https://github.com/DoctorMcKay/node-steam-crypto

#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate lazy_static_include;

use std::fs::OpenOptions;
use std::io::Read;

use openssl::error::ErrorStack;
use openssl::hash::MessageDigest;
use openssl::rsa::{Padding, Rsa, RsaRef};
use openssl::sha::Sha1;
use openssl::sign::Verifier;
use openssl::symm::{Cipher, Crypter, Mode};
use rand::prelude::*;

mod symm;
use symm::*;

lazy_static_include_bytes!(STEAM_KEY, "assets/steam_public.pem");

#[derive(Debug)]
pub struct SessionKeys {
    pub plain_text: Vec<u8>,
    pub encrypted: Vec<u8>,
}

pub fn verify_signature(data: &[u8], signature: &[u8]) -> Result<bool, ErrorStack> {
    /// standard algorithm is RSA-SHA1
    /// but this should be selectable
    let steam_key_bytes: &'static [u8] = *STEAM_KEY;
    let steam_pkey = openssl::pkey::PKey::public_key_from_pem(&steam_key_bytes).unwrap();

    let mut verifier = Verifier::new(MessageDigest::sha1(), &steam_pkey).unwrap();
    verifier.update(&data).unwrap();
    verifier.verify(&signature)
}

/// Generates a 32 byte random blob of data and encrypts it with RSA using the Steam system's public key.
/// Returns SessionsKeys struct.
pub fn generate_session_key(nonce: &str) -> SessionKeys {
    /// If there is a nonce, it gets concatenated after the generated 32 bytes
    let mut random_bytes_array: Vec<u8> = vec![0; 32];
    let mut encrypted_array: Vec<u8> = vec![0; 256];

    thread_rng().fill_bytes(&mut random_bytes_array);

    random_bytes_array.extend(nonce.bytes());

    let steam_key: &'static [u8] = *STEAM_KEY;
    let public_key = openssl::rsa::Rsa::public_key_from_pem(&steam_key).unwrap();
    RsaRef::public_encrypt(&public_key, &random_bytes_array, &mut encrypted_array, Padding::PKCS1)
        .unwrap();

    SessionKeys { plain_text: random_bytes_array, encrypted: encrypted_array }
}

#[cfg(test)]
mod tests {
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use crate::generate_session_key;

    #[test]
    fn it_works() {
        println!("{:?}", generate_session_key(&"kkkkkkkkkk").plain_text)
    }
}
