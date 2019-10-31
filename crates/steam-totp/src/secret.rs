use base64;
use hex;


#[derive(Debug)]
struct SecretInner {
    data: Vec<u8>,
}

/// Struct for working with your TOTP shared secret.
#[derive(Debug)]
pub struct Secret(SecretInner);

impl Secret {
    /// Creates a new Secret from a raw byte slice.
    pub fn new(secret: &[u8]) -> Secret {
        Secret(SecretInner {
            data: secret.to_vec(),
        })
    }

    /// Creates a new Secret from a hex encoded string.
    pub fn from_hex(secret: &str) -> Result<Secret, hex::FromHexError> {
        Ok(Secret(SecretInner {
            data: hex::decode(secret)?
        }))
    }

    /// Creates a new Secret from a base64 encoded string.
    pub fn from_b64(secret: &str) -> Result<Secret, base64::DecodeError> {
        Ok(Secret(SecretInner {
            data: base64::decode(secret)?
        }))
    }

    pub(crate) fn data<'a>(&'a self) -> &[u8] {
        &self.0.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_raw_secret() -> Vec<u8> {
        hex::decode("deadbeefcafe00").unwrap()
    }


    #[test]
    fn secret_new() {
        let raw = make_raw_secret();
        let secret = Secret::new(&raw);

        assert_eq!(secret.0.data, raw);
    }

    #[test]
    fn secret_from_hex() {
        let raw = make_raw_secret();
        let hex_str = hex::encode(&raw);
        let secret = Secret::from_hex(&hex_str);

        assert_eq!(secret.is_ok(), true);

        let secret = secret.unwrap();
        assert_eq!(secret.data(), &raw[..]);
    }

    #[test]
    fn secret_from_b64() {
        let raw = make_raw_secret();
        let b64_str = base64::encode(&raw);
        let secret = Secret::from_b64(&b64_str);

        assert_eq!(secret.is_ok(), true);

        let secret = secret.unwrap();
        assert_eq!(secret.data(), &raw[..]);
    }

    #[test]
    fn secret_data() {
        let raw = make_raw_secret();
        let secret = Secret::new(&raw);

        assert_eq!(secret.data(), &raw[..]);
    }

}
