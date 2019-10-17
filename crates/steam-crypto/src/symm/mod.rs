use openssl::symm::{Cipher, Crypter, Mode};
use rand::{thread_rng, RngCore};

//fn cipher_message(message: &[u8], key: &[u8], plain_iv: Option<&[u8]>, mode: Mode) -> Vec<u8> {
//    let mut message_crypter = Crypter::new(Cipher::aes_256_cbc(), mode, key, plain_iv);
//    match plain_iv {
//        None => {}
//        Some(_) => message_crypter.unwrap().pad(false),
//    }
//
//    let mut buffer = message_crypter.buffer.extend(message_crypter.finalize().into_iter());
//    buffer
//}

/// Encrypt or decrypt an Initialization Vector with AES 256 ECB.
fn cipher_iv_ecb(key: &[u8], plain_iv: Option<&[u8]>, mode: Mode) -> Vec<u8> {
    let mut iv_cipher = Crypter::new(Cipher::aes_256_ecb(), mode, key, plain_iv).unwrap();
    iv_cipher.pad(false);

    let mut output_buffer: Vec<u8> = Vec::new();

    iv_cipher.update(plain_iv.unwrap(), &mut output_buffer);
    iv_cipher.finalize(&mut output_buffer).unwrap();
    output_buffer
}

pub fn symetric_encrypt() {
    let mut iv: [u8; 16] = [0; 16];
    thread_rng().fill_bytes(&mut iv);
}

pub fn symmetric_decrypt(input: &[u8], key: &[u8]) -> Vec<u8> {
    let encrypted_iv = &input[0..16];
    let plain_iv = cipher_iv_ecb(key, Option::from(encrypted_iv), Mode::Decrypt);

    let encrypted_message = &input[16..];
    //    let decrypted_message = crypt_message(encrypted_message, key, &plain_iv, Mode::Decrypt);

    //    decrypted_message
    plain_iv
}
