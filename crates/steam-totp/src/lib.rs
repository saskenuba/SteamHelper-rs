//! Direct port of
//! https://github.com/DoctorMcKay/node-steam-totp/blob/master/index.js
extern crate bytes;
extern crate hmac;
extern crate sha1;

mod error;

pub use error::TotpError;

use bytes::{BigEndian,ByteOrder};
use hmac::{Hmac,Mac};
use sha1::Sha1;
use std::{
    result,
    time::{SystemTime,UNIX_EPOCH},
};


/// A `Result` wrapper for totp operations.
pub type Result<T> = result::Result<T, TotpError>;
type HmacSha1 = Hmac<Sha1>;


/// Generate a Steam-style TOTP authentication code.
pub fn generate_auth_code(secret: &[u8], offset: Option<u64>) -> Result<String> {
    let time = time(offset)?;
    let buf = create_initial_auth_buffer(time);
    let hmac = create_hmac_for_auth(secret, &buf)?;
    let fullcode = create_fullcode_for_auth(&hmac);

    Ok(derive_auth_code(fullcode))
}

fn time(offset: Option<u64>) -> Result<u64> {
    let offset = offset.unwrap_or(0);
    let unix_time_secs = SystemTime::now().duration_since(UNIX_EPOCH)?
        .as_secs();

    Ok(offset + unix_time_secs)
}

fn create_initial_auth_buffer(time: u64) -> [u8; 8] {
    let mut buf = [0; 8];
    BigEndian::write_u32(&mut buf[4..], time as u32 / 30);
    buf
}

fn create_hmac_for_auth<'a>(secret: &'a [u8], buffer: &[u8]) -> Result<Vec<u8>> {
    let mut hmac = HmacSha1::new_varkey(secret)?;

    hmac.input(buffer);
    Ok(hmac.result().code().as_slice().to_vec())
}

fn create_fullcode_for_auth(hmac: &[u8]) -> usize {
    let start = hmac[19] as usize & 0x0F;
    let code = &hmac[start..start + 4];

    BigEndian::read_u32(&code) as usize & 0x7FFFFFFF
}

fn derive_auth_code(fullcode: usize) -> String {
    let char_set = "23456789BCDFGHJKMNPQRTVWXY";

    (0..5).fold((String::new(), fullcode), |(mut code, fullcode), _| {
        let c = char_set.chars()
            .nth(fullcode % char_set.len())
            .unwrap(); // We modulo, so this can't panic

        code.push(c);
        (code, fullcode / char_set.len())
    })
    .0
}

/// TODO: Add doc
pub fn generate_confirmation_key() {
    unimplemented!()
}

/// TODO: Add doc
pub fn get_device_id() {
    unimplemented!()
}

/// TODO: Add doc
pub fn get_time_offset() {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_secret() -> [u8; 8] {
        let mut secret = [0; 8];
        BigEndian::write_u64(&mut secret, 62_678_480_394_959_550);
        secret
    }

    #[test]
    fn time_returns_seconds() {
        use std::time::{SystemTime,UNIX_EPOCH};

        let now_seconds = SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        assert_eq!(time(None).unwrap(), now_seconds);
    }

    #[test]
    fn time_returns_seconds_with_offset() {
        let offset = 100;
        let now_seconds = SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        assert_eq!(time(Some(offset)).unwrap(), now_seconds + offset);
    }

    #[test]
    fn create_initial_auth_buffer_succeeds() {
        let buffer = create_initial_auth_buffer(9001);
        assert_eq!(BigEndian::read_u64(&buffer), 300);
    }

    #[test]
    fn create_hmac_for_auth_succeeds() {
        let secret = make_secret();
        let buf = create_initial_auth_buffer(9001);
        let hmac = create_hmac_for_auth(&secret, &buf).unwrap();

        let as_hex_string = |xs: Vec<u8>| xs.into_iter()
            .map(|x| format!("{:x?}", x))
            .collect::<String>();

        assert_eq!(
            as_hex_string(hmac),
            "c32862db5db4eeb121bc9cc267a416b9c515db7".to_owned(),
        );
    }

    #[test]
    fn create_fullcode_for_auth_succeeds() {
        let secret = make_secret();
        let buf = create_initial_auth_buffer(9001);
        let hmac = create_hmac_for_auth(&secret, &buf).unwrap();

        assert_eq!(create_fullcode_for_auth(&hmac), 824294556);
    }

    #[test]
    fn derive_auth_code_succeeds() {
        let secret = make_secret();
        let buf = create_initial_auth_buffer(9001);
        let hmac = create_hmac_for_auth(&secret, &buf).unwrap();
        let fullcode = create_fullcode_for_auth(&hmac);

        assert_eq!(derive_auth_code(fullcode), String::from("RMVRC"));
    }
}
