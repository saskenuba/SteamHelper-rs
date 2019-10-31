use base64;
use hmac::crypto_mac::InvalidKeyLength;
use hex::FromHexError;
use std::{
    error,
    fmt,
    time::SystemTimeError,
};

/// A custom `Error` for totp operations that wraps underlying errors.
#[derive(Debug)]
pub enum TotpError {
    Time(SystemTimeError),
    Hmac(InvalidKeyLength),
    HexDecode(FromHexError),
    B64Decode(base64::DecodeError),
}

impl fmt::Display for TotpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TotpError::Time(ref err) => write!(f, "SystemTime error: {}", err),
            TotpError::Hmac(ref err) => write!(f, "Hmac error: {}", err),
            TotpError::HexDecode(ref err) => write!(f, "FromHex error: {}", err),
            TotpError::B64Decode(ref err) => write!(f, "Base64 error: {}", err),
        }
    }
}

impl error::Error for TotpError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            TotpError::Time(ref err) => Some(err),
            TotpError::Hmac(ref err) => Some(err),
            TotpError::HexDecode(ref err) => Some(err),
            TotpError::B64Decode(ref err) => Some(err),
        }
    }
}

impl From<SystemTimeError> for TotpError {
    fn from(err: SystemTimeError) -> TotpError {
        TotpError::Time(err)
    }
}

impl From<InvalidKeyLength> for TotpError {
    fn from(err: InvalidKeyLength) -> TotpError {
        TotpError::Hmac(err)
    }
}

impl From<FromHexError> for TotpError {
    fn from(err: FromHexError) -> TotpError {
        TotpError::HexDecode(err)
    }
}

impl From<base64::DecodeError> for TotpError {
    fn from(err: base64::DecodeError) -> TotpError {
        TotpError::B64Decode(err)
    }
}
