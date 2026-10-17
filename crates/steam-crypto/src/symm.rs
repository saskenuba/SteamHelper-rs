//! AES-256 encryption of CM packets.
//!
//! A ciphertext is the IV encrypted with AES-256-ECB (16 bytes), followed by the message encrypted with AES-256-CBC and
//! PKCS#7 padding under that IV. Both use the session key.

use std::convert::TryFrom;
use std::ops::Range;

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::BlockDecrypt;
use aes::cipher::BlockDecryptMut;
use aes::cipher::BlockEncrypt;
use aes::cipher::BlockEncryptMut;
use aes::cipher::KeyInit;
use aes::cipher::KeyIvInit;
use aes::Aes256;
use hmac::Hmac;
use hmac::Mac;
use rand::thread_rng;
use rand::RngCore;
use sha1::Sha1;

use crate::CryptoError;
use crate::Result;
use crate::SessionKey;

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;
type HmacSha1 = Hmac<Sha1>;

const BLOCK_SIZE: usize = 16;
/// Random bytes at the end of an HMAC IV.
const HMAC_RANDOM_SIZE: usize = 3;
/// HMAC bytes at the start of an HMAC IV.
const HMAC_PREFIX_SIZE: usize = BLOCK_SIZE - HMAC_RANDOM_SIZE;
/// The HMAC secret is the first half of the session key.
const HMAC_SECRET_SIZE: usize = 16;

/// Encrypts `input` with a random IV.
pub fn symmetric_encrypt(input: &[u8], key: &SessionKey) -> Vec<u8> {
    let mut iv = [0u8; BLOCK_SIZE];
    thread_rng().fill_bytes(&mut iv);
    symmetric_encrypt_with_iv(input, key, &iv)
}

/// Encrypts `input` with the given IV.
pub fn symmetric_encrypt_with_iv(input: &[u8], key: &SessionKey, iv: &[u8; BLOCK_SIZE]) -> Vec<u8> {
    let key = key.as_bytes().into();

    let mut encrypted_iv = (*iv).into();
    Aes256::new(key).encrypt_block(&mut encrypted_iv);

    let encrypted_message = Aes256CbcEnc::new(key, iv.into()).encrypt_padded_vec_mut::<Pkcs7>(input);

    let mut output = Vec::with_capacity(BLOCK_SIZE + encrypted_message.len());
    output.extend_from_slice(&encrypted_iv);
    output.extend_from_slice(&encrypted_message);
    output
}

/// Encrypts `input` with an IV that authenticates it, the way CM sessions are encrypted.
///
/// The IV is the first 13 bytes of `HMAC-SHA1(key[..16], random || input)` followed by the 3 `random` bytes.
pub fn symmetric_encrypt_hmac_iv(input: &[u8], key: &SessionKey) -> Vec<u8> {
    let mut random = [0u8; HMAC_RANDOM_SIZE];
    thread_rng().fill_bytes(&mut random);
    encrypt_hmac_iv_with_random(input, key, random)
}

fn encrypt_hmac_iv_with_random(input: &[u8], key: &SessionKey, random: [u8; HMAC_RANDOM_SIZE]) -> Vec<u8> {
    let hmac = hmac_sha1(key, &random, input);

    let mut iv = [0u8; BLOCK_SIZE];
    iv[..HMAC_PREFIX_SIZE].copy_from_slice(&hmac[..HMAC_PREFIX_SIZE]);
    iv[HMAC_PREFIX_SIZE..].copy_from_slice(&random);

    symmetric_encrypt_with_iv(input, key, &iv)
}

/// Decrypts a ciphertext made by [`symmetric_encrypt`] or [`symmetric_encrypt_with_iv`].
pub fn symmetric_decrypt(input: &[u8], key: &SessionKey) -> Result<Vec<u8>> {
    let mut buffer = input.to_vec();
    let message = symmetric_decrypt_in_place(&mut buffer, key)?;
    Ok(into_message(buffer, message))
}

/// Decrypts a ciphertext made by [`symmetric_encrypt_hmac_iv`], and checks the HMAC carried in its IV.
pub fn symmetric_decrypt_hmac_iv(input: &[u8], key: &SessionKey) -> Result<Vec<u8>> {
    let mut buffer = input.to_vec();
    let message = symmetric_decrypt_hmac_iv_in_place(&mut buffer, key)?;
    Ok(into_message(buffer, message))
}

/// Decrypts, in place, a ciphertext made by [`symmetric_encrypt`] or [`symmetric_encrypt_with_iv`].
///
/// Returns where the plain message is in `buffer`: after the IV, and before the padding. The rest of `buffer` is left
/// with the plain IV and padding.
pub fn symmetric_decrypt_in_place(buffer: &mut [u8], key: &SessionKey) -> Result<Range<usize>> {
    decrypt_in_place(buffer, key).map(|(_, message)| message)
}

/// Decrypts, in place, a ciphertext made by [`symmetric_encrypt_hmac_iv`], and checks the HMAC carried in its IV.
///
/// Returns where the plain message is in `buffer`, as [`symmetric_decrypt_in_place`]. Doesn't allocate.
pub fn symmetric_decrypt_hmac_iv_in_place(buffer: &mut [u8], key: &SessionKey) -> Result<Range<usize>> {
    let (iv, message) = decrypt_in_place(buffer, key)?;
    let (hmac_prefix, random) = iv.split_at(HMAC_PREFIX_SIZE);

    let mut mac = hmac_sha1_mac(key);
    mac.update(random);
    mac.update(&buffer[message.clone()]);
    mac.verify_truncated_left(hmac_prefix)
        .map_err(|_| CryptoError::InvalidHmac)?;

    Ok(message)
}

/// Keeps only the plain message of a buffer decrypted in place.
fn into_message(mut buffer: Vec<u8>, message: Range<usize>) -> Vec<u8> {
    buffer.truncate(message.end);
    buffer.drain(..message.start);
    buffer
}

/// Returns the plain IV, and where the plain message is in `buffer`.
fn decrypt_in_place(buffer: &mut [u8], key: &SessionKey) -> Result<([u8; BLOCK_SIZE], Range<usize>)> {
    if buffer.len() < 2 * BLOCK_SIZE || !buffer.len().is_multiple_of(BLOCK_SIZE) {
        return Err(CryptoError::InvalidLength(buffer.len()));
    }
    let (encrypted_iv, encrypted_message) = buffer.split_at_mut(BLOCK_SIZE);
    let key = key.as_bytes().into();

    let mut iv = aes::Block::from(<[u8; BLOCK_SIZE]>::try_from(&*encrypted_iv).expect("split at BLOCK_SIZE"));
    Aes256::new(key).decrypt_block(&mut iv);
    encrypted_iv.copy_from_slice(&iv);

    let message_len = Aes256CbcDec::new(key, &iv)
        .decrypt_padded_mut::<Pkcs7>(encrypted_message)
        .map_err(|_| CryptoError::InvalidPadding)?
        .len();

    Ok((iv.into(), BLOCK_SIZE..BLOCK_SIZE + message_len))
}

fn hmac_sha1_mac(key: &SessionKey) -> HmacSha1 {
    <HmacSha1 as Mac>::new_from_slice(&key.as_bytes()[..HMAC_SECRET_SIZE]).expect("HMAC accepts keys of any size")
}

fn hmac_sha1(key: &SessionKey, random: &[u8], input: &[u8]) -> [u8; 20] {
    let mut mac = hmac_sha1_mac(key);
    mac.update(random);
    mac.update(input);
    mac.finalize().into_bytes().into()
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;

    const MESSAGE: &[u8] = b"The quick brown fox jumps over the lazy dog";

    fn key() -> SessionKey {
        let mut key = [0u8; 32];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = i as u8;
        }
        SessionKey::from(key)
    }

    fn unhex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    // Expected bytes below come from the `openssl` CLI:
    // `openssl enc -aes-256-ecb -nopad` for the IV, `openssl enc -aes-256-cbc` for the message,
    // and `openssl mac -digest SHA1 ... HMAC` for the HMAC.

    #[test]
    fn encrypt_with_iv_matches_openssl() {
        let iv: [u8; 16] = unhex("f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff").try_into().unwrap();
        let expected = unhex(concat!(
            "9200cd8d239680cb5a69e65440326314",
            "84f474f35a3090a82aae847eff728ab2c6d8dc5cfb3695e68937cb12ec50264e0a87c148ad2f36a22c43522e3ec511a5",
        ));

        let encrypted = symmetric_encrypt_with_iv(MESSAGE, &key(), &iv);
        assert_eq!(encrypted, expected);
        assert_eq!(symmetric_decrypt(&encrypted, &key()).unwrap(), MESSAGE);
    }

    #[test]
    fn encrypt_hmac_iv_matches_openssl() {
        let expected = unhex(concat!(
            "904aed0ca555cfc0cd645ad245ef7a16",
            "9d5166d0a685160331b90d5e4ea1922810200fa712e05120ddbd2c1baa2f75e307416f65f4a457c2c252988f57879472",
        ));

        let encrypted = encrypt_hmac_iv_with_random(MESSAGE, &key(), *b"abc");
        assert_eq!(encrypted, expected);
        assert_eq!(symmetric_decrypt_hmac_iv(&encrypted, &key()).unwrap(), MESSAGE);
    }

    #[test]
    fn decrypts_in_place() {
        let mut buffer = encrypt_hmac_iv_with_random(MESSAGE, &key(), *b"abc");
        let message = symmetric_decrypt_hmac_iv_in_place(&mut buffer, &key()).unwrap();
        assert_eq!(message, 16..16 + MESSAGE.len());
        assert_eq!(&buffer[message], MESSAGE);
        // the plain IV: the HMAC prefix, then the random bytes
        assert_eq!(&buffer[13..16], b"abc");

        let iv = [0xF0; 16];
        let mut buffer = symmetric_encrypt_with_iv(b"", &key(), &iv);
        let message = symmetric_decrypt_in_place(&mut buffer, &key()).unwrap();
        assert_eq!(message, 16..16);
        assert_eq!(buffer[..16], iv);
    }

    #[test]
    fn round_trips() {
        for message in [&b""[..], b"a", &[0u8; 16], &[0xFF; 1000]] {
            let encrypted = symmetric_encrypt_hmac_iv(message, &key());
            assert_eq!(symmetric_decrypt_hmac_iv(&encrypted, &key()).unwrap(), message);

            let encrypted = symmetric_encrypt(message, &key());
            assert_eq!(symmetric_decrypt(&encrypted, &key()).unwrap(), message);
        }
    }

    #[test]
    fn rejects_tampered_message() {
        let mut encrypted = symmetric_encrypt_hmac_iv(MESSAGE, &key());
        // Flipping a bit of the first ciphertext block flips the same bit of the second plaintext block,
        // leaving the padding intact, so only the HMAC can catch it.
        encrypted[16] ^= 1;
        assert_eq!(
            symmetric_decrypt_hmac_iv(&encrypted, &key()),
            Err(CryptoError::InvalidHmac)
        );
    }

    #[test]
    fn rejects_message_without_hmac() {
        let encrypted = symmetric_encrypt(MESSAGE, &key());
        assert_eq!(
            symmetric_decrypt_hmac_iv(&encrypted, &key()),
            Err(CryptoError::InvalidHmac)
        );
    }

    #[test]
    fn rejects_wrong_key() {
        let encrypted = symmetric_encrypt_hmac_iv(MESSAGE, &key());
        let other_key = SessionKey::from([0x42; 32]);
        assert!(symmetric_decrypt_hmac_iv(&encrypted, &other_key).is_err());
    }

    #[test]
    fn rejects_bad_lengths() {
        for len in [0, 15, 16, 31, 33] {
            assert_eq!(
                symmetric_decrypt(&vec![0; len], &key()),
                Err(CryptoError::InvalidLength(len))
            );
            assert_eq!(
                symmetric_decrypt_hmac_iv_in_place(&mut vec![0; len], &key()),
                Err(CryptoError::InvalidLength(len))
            );
        }
    }
}
