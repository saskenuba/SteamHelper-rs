
use openssl::{
    error::ErrorStack,
    hash::MessageDigest,
    sign::Signer,
    symm::{Cipher, Crypter, Mode},
};
use rand::{thread_rng, RngCore};

type Result<T> = std::result::Result<T, ErrorStack>;

/// Encrypt or decrypt a message with AES 256 CBC.
pub fn cipher_message(
    message: &[u8],
    key: &[u8],
    plain_iv: Option<&[u8]>,
    mode: Mode,
) -> Result<Vec<u8>> {
    let mut message_cipher = Crypter::new(Cipher::aes_256_cbc(), mode, key, plain_iv)?;

    let mut output_buffer: Vec<u8> = Vec::new();

    message_cipher.update(&message, &mut output_buffer)?;
    message_cipher.finalize(&mut output_buffer)?;
    Ok(output_buffer)
}

/// Encrypt or decrypt an Initialization Vector with AES 256 ECB.
fn cipher_iv_ecb(key: &[u8], plain_iv: Option<&[u8]>, mode: Mode) -> Result<Vec<u8>> {
    let mut iv_cipher = Crypter::new(Cipher::aes_256_ecb(), mode, key, plain_iv).unwrap();
    iv_cipher.pad(false);

    let mut output_buffer: Vec<u8> = Vec::new();

    iv_cipher.update(plain_iv.unwrap(), &mut output_buffer)?;
    iv_cipher.finalize(&mut output_buffer).unwrap();
    Ok(output_buffer)
}

pub fn symmetric_encrypt(input: &[u8], key: &[u8]) -> Vec<u8> {
    let mut iv: [u8; 16] = [0; 16];
    thread_rng().fill_bytes(&mut iv);

    symmetric_encrypt_with_iv(input, key, Option::from(&iv[..])).unwrap()
}

pub fn symmetric_encrypt_with_iv(
    message: &[u8],
    key: &[u8],
    plain_iv: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let encrypted_iv = cipher_iv_ecb(key, plain_iv, Mode::Encrypt)?;
    let encrypted_message = cipher_message(message, key, plain_iv, Mode::Encrypt)?;

    let mut output = encrypted_iv;
    output.extend(encrypted_message.into_iter());
    Ok(output)
}

pub fn symmetric_decrypt(input: &[u8], key: &[u8], is_hmac: bool) -> Result<Vec<u8>> {
    let encrypted_iv = &input[0..16];
    let plain_iv = cipher_iv_ecb(key, Some(encrypted_iv), Mode::Decrypt)?;

    let encrypted_message = &input[16..];

    if !is_hmac {
        cipher_message(encrypted_message, key, Some(encrypted_iv), Mode::Decrypt)?;
    }
    let iv_len = plain_iv.len();
    let hmac_partial = &plain_iv[..iv_len - 3];
    let hmac_random_bytes = &plain_iv[iv_len - 3..];

    let signed_data = sign_hmac_sha1(&hmac_random_bytes, &plain_iv, &key[..16]).unwrap();
    if &signed_data[..iv_len] != hmac_partial {
        panic!("Received invalid HMAC from remote host.");
    }
    Ok(plain_iv)
}

/// Encrypt input with key. Returns HMAC
/// IV is HMAC-SHA1(Random(3) + Plaintext) + Random(3). (Same random values for both)
pub fn symmetric_encrypt_hmac_iv(input: &[u8], key: &[u8]) -> Vec<u8> {
    const RAND_VEC_SIZE: usize = 3;
    let mut random_vec: [u8; RAND_VEC_SIZE] = [0; RAND_VEC_SIZE];
    thread_rng().fill_bytes(&mut random_vec);

    let signed_data = sign_hmac_sha1(&mut random_vec, &input, &key[..16]).unwrap();

    // the resulting IV must be 16 bytes long, so truncate the hmac to make room for the random
    let mut signed_data_slice = signed_data[..16 - RAND_VEC_SIZE].to_vec();
    signed_data_slice.extend(random_vec.iter());

    symmetric_encrypt_with_iv(input, key, Some(signed_data_slice.as_ref())).unwrap()
}

fn sign_hmac_sha1(random_bytes: &[u8], input: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    let pkey = openssl::pkey::PKey::hmac(&key)?;
    let mut signer = Signer::new(MessageDigest::sha1(), &pkey)?;
    signer.update(random_bytes)?;
    signer.update(&input)?;
    let signed_data = signer.sign_to_vec()?;
    Ok(signed_data)
}
