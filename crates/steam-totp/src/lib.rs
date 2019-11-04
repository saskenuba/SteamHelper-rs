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
//!     let shared_secret = Secret::from_hex("deadbeefcafe").unwrap();
//!     let auth_code = generate_auth_code(shared_secret, time);
//!
//!     println!("{}", auth_code);  // Will print a 5 character code similar to "R7VRC"
//! }
//! ```

pub extern crate base64;
pub extern crate hex;
pub extern crate hmac;
pub extern crate sha1;

pub use error::TotpError;
pub use secret::Secret;
pub use time::Time;

use bytes::{BigEndian, ByteOrder};
use sha1::{Digest, Sha1};
use std::result;

mod error;
mod time;
mod secret;

pub type Result<T> = result::Result<T, TotpError>;

/// Generate a Steam TOTP authentication code.
///
/// `offset` is the difference of time in seconds that your server is off from
/// the steam servers.
///
/// **Note:** You should use your `shared_secret` for this.
pub fn generate_auth_code(mut shared_secret: Secret, time: Time) -> String {
    fn create_fullcode(hmac: &[u8]) -> usize {
        let start = hmac[19] as usize & 0x0F;
        let code = &hmac[start..start + 4];

        BigEndian::read_u32(&code) as usize & 0x7FFFFFFF
    }

    fn derive_2fa_code(fullcode: usize) -> String {
        let char_set = "23456789BCDFGHJKMNPQRTVWXY";

        (0..5)
            .fold((String::new(), fullcode), |(mut code, fullcode), _| {
                // We modulo, so this can't panic
                let c = char_set.chars().nth(fullcode % char_set.len()).unwrap();

                code.push(c);
                (code, fullcode / char_set.len())
            })
            .0
    }

    let buffer = time.as_padded_buffer(Some(30));
    let digest = shared_secret.hmac_input(&buffer).code_as_vec();
    let fullcode = create_fullcode(&digest);

    derive_2fa_code(fullcode)
}

/// Returns a string containing your base64 confirmation key for use with the
/// mobile confirmations web page.
///
/// `tag` identifies what this request (and therefore key) will be for.
/// `"conf"` to load the confirmations page, `"details"` to load details about a
/// trade, `"allow"` to confirm a trade, `"cancel"` to cancel it.
///
/// **Note:** You should use your `shared_secret` for this.
pub fn generate_confirmation_key(
    mut identity_secret: Secret,
    time: Time,
    tag: Option<&str>,
) -> Result<String> {
    fn create_buffer(time: Time, tag: Option<&str>) -> Vec<u8> {
        let mut buffer = time.as_padded_buffer(None);

        if let Some(x) = tag {
            let mut tag = x.as_bytes().into_iter().take(32).map(|x| x.to_owned()).collect();
            buffer.append(&mut tag);
        }
        buffer
    }

    let buffer = create_buffer(time, tag);
    Ok(identity_secret.hmac_input(&buffer).code())
}

/// Get a standardized device ID based on your SteamID.
pub fn get_device_id(steam_id: &str) -> String {
    let hash = Sha1::digest(steam_id.as_bytes());
    let hex = hex::encode(hash);

    let (one, rest) = hex.split_at(8);
    let (two, rest) = rest.split_at(4);
    let (three, rest) = rest.split_at(4);
    let (four, rest) = rest.split_at(4);
    let (five, _) = rest.split_at(12);

    format!("android:{}-{}-{}-{}-{}", one, two, three, four, five)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fixed_time() -> Time {
        Time(1572580000)
    }

    fn make_secret() -> Secret {
        let raw = hex::decode("deadbeefcafe00").unwrap();
        Secret::new(&raw).unwrap()
    }

    #[test]
    pub fn generate_auth_code_succeeds() {
        let secret = make_secret();
        let time = make_fixed_time();

        assert_eq!(generate_auth_code(secret, time), "6RFHH");
    }

    #[test]
    fn generate_confirmation_key_without_tag_succeeds() {
        let time = make_fixed_time();
        let secret = make_secret();

        assert_eq!(
            generate_confirmation_key(secret, time, None).unwrap(),
            "Y3orSQpLIsycZY/6shH8j/1UwRY=".to_string()
        );
    }

    #[test]
    fn generate_confirmation_key_with_tag_succeeds() {
        let time = make_fixed_time();
        let secret = make_secret();

        assert_eq!(
            generate_confirmation_key(secret, time, Some("details")).unwrap(),
            "uofPzqUhpWkuPH4ZWuRSWejdfAw=".to_string()
        );
    }

    #[test]
    fn get_device_id_succeeds() {
        let steam_id = "myWonderfulSteamId";

        assert_eq!(
            get_device_id(steam_id),
            "android:cd5f79f7-6eb7-77fb-f3c6-211c848cf6d1".to_string()
        );
    }
}
