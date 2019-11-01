//! Direct port of
//! [DoctorMcKay/node-steam-totp](https://github.com/DoctorMcKay/node-steam-totp)
//!
//! This crate generates Steam 2FA auth codes for a shared secret.
//!
//! # Example
//!
//! ```
//! use steam_totp::{Time,Secret,generate_auth_code};
//!
//! fn main() {
//!     let time = Time::now(None).unwrap();
//!     let secret = Secret::from_hex("deadbeefcafe").unwrap();
//!     let auth_code = generate_auth_code(secret, time).unwrap();
//!
//!     println!("{}", auth_code);  // Will print a 5 character code similar to "R7VRC"
//! }
//! ```

mod time;
mod secret;

pub use time::Time;
pub use secret::Secret;

use bytes::{BigEndian,ByteOrder};
use hmac::{crypto_mac::InvalidKeyLength,Hmac,Mac};
use sha1::Sha1;


type HmacSha1 = Hmac<Sha1>;


/// Generate a Steam TOTP authentication code.
///
/// `offset` is the difference of time in seconds that your server is off from
/// the steam servers.
pub fn generate_auth_code(secret: Secret, time: Time) -> Result<String, InvalidKeyLength> {
    let buf = create_initial_auth_buffer(time.time());
    let hmac = create_hmac_for_auth(secret.data(), &buf)?;
    let fullcode = create_fullcode_for_auth(&hmac);

    Ok(derive_auth_code(fullcode))
}

fn create_initial_auth_buffer(time: u64) -> [u8; 8] {
    let mut buf = [0; 8];
    BigEndian::write_u32(&mut buf[4..], time as u32 / 30);
    buf
}

fn create_hmac_for_auth<'a>(secret: &'a [u8], buffer: &[u8]) -> Result<Vec<u8>, InvalidKeyLength> {
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

    fn make_raw_secret() -> Vec<u8> {
        hex::decode("deadbeefcafe00").unwrap()
    }

    fn make_secret() -> Secret {
        let raw = make_raw_secret();
        Secret::new(&raw)
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
        let hmac = create_hmac_for_auth(secret.data(), &buf).unwrap();

        let as_hex_string = |xs: Vec<u8>| xs.into_iter()
            .map(|x| format!("{:x?}", x))
            .collect::<String>();

        assert_eq!(
            as_hex_string(hmac),
            "e73054a5397bbbabbd20ff4655d3cd79d8425359".to_owned(),
        );
    }

    #[test]
    fn create_fullcode_for_auth_succeeds() {
        let secret = make_secret();
        let buf = create_initial_auth_buffer(9001);
        let hmac = create_hmac_for_auth(secret.data(), &buf).unwrap();

        assert_eq!(create_fullcode_for_auth(&hmac), 553600597);
    }

    #[test]
    fn derive_auth_code_succeeds() {
        let secret = make_secret();
        let buf = create_initial_auth_buffer(9001);
        let hmac = create_hmac_for_auth(secret.data(), &buf).unwrap();
        let fullcode = create_fullcode_for_auth(&hmac);

        assert_eq!(derive_auth_code(fullcode), String::from("NRHFK"));
    }
}
