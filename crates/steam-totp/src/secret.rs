use super::Result;

use hmac::{Hmac, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

/// Struct for working with your TOTP shared secret.
#[derive(Debug, Clone)]
pub struct Secret {
    value: Vec<u8>,
    hmac: HmacSha1,
}

impl Secret {
    /// Creates a new Secret from a raw byte slice.
    pub fn new(secret: &[u8]) -> Result<Secret> {
        Ok(Secret {
            value: secret.to_vec(),
            hmac: HmacSha1::new_from_slice(&secret)?,
        })
    }

    /// Creates a new Secret from a hex encoded string.
    pub fn from_hex(secret: &str) -> Result<Secret> {
        let value = hex::decode(secret)?;
        Ok(Secret {
            value: value.clone(),
            hmac: HmacSha1::new_from_slice(&value)?,
        })
    }

    /// Creates a new Secret from a base64 encoded string.
    pub fn from_b64(secret: &str) -> Result<Secret> {
        let value = base64::decode(secret)?;
        Ok(Secret {
            value: value.clone(),
            hmac: HmacSha1::new_from_slice(&value)?,
        })
    }

    pub(crate) fn code(&self) -> String {
        base64::encode(&self.code_as_vec())
    }

    pub(crate) fn code_as_vec(&self) -> Vec<u8> {
        self.hmac.clone().finalize().into_bytes().to_vec()
    }

    pub(crate) fn hmac_input(&mut self, data: &[u8]) -> &Self {
        self.hmac.update(data);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_raw_secret() -> Vec<u8> {
        hex::decode("deadbeefcafe00").unwrap()
    }

    fn make_secret() -> Secret {
        let raw = make_raw_secret();
        Secret::new(&raw).unwrap()
    }

    #[test]
    fn secret_new() {
        let raw = make_raw_secret();
        let secret = Secret::new(&raw).unwrap();

        assert_eq!(secret.value, &raw[..]);
    }

    #[test]
    fn secret_from_hex() {
        let raw = make_raw_secret();
        let hex_str = hex::encode(&raw);
        let secret = Secret::from_hex(&hex_str);

        assert_eq!(secret.is_ok(), true);

        let secret = secret.unwrap();
        assert_eq!(secret.value, &raw[..]);
    }

    #[test]
    fn secret_from_b64() {
        let raw = make_raw_secret();
        let b64_str = base64::encode(&raw);
        let secret = Secret::from_b64(&b64_str);

        assert_eq!(secret.is_ok(), true);

        let secret = secret.unwrap();
        assert_eq!(secret.value, &raw[..]);
    }

    #[test]
    fn secret_code() {
        let secret = make_secret();
        let hmac = HmacSha1::new_from_slice(&secret.value).unwrap();
        let expected = base64::encode(&hmac.finalize().into_bytes());

        assert_eq!(secret.code(), expected);
    }

    #[test]
    fn secret_code_as_vec() {
        let secret = make_secret();
        let hmac = HmacSha1::new_from_slice(&secret.value).unwrap();
        let expected = hmac.finalize().into_bytes().to_vec();

        assert_eq!(secret.code_as_vec(), expected);
    }

    #[test]
    fn hmac_input() {
        let mut secret = make_secret();
        let mut hmac = HmacSha1::new_from_slice(&secret.value).unwrap();
        let data = b"b000";

        hmac.update(&data[..]);
        let expected = hmac.finalize().into_bytes().to_vec();

        assert_eq!(secret.hmac_input(data).code_as_vec(), expected);
    }
}
