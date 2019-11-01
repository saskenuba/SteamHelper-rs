use base64;
use hex;
use hmac::crypto_mac::InvalidKeyLength;
use std::{error, fmt, time::SystemTimeError};

/// The error type for TOTP operations that wraps underlying errors.
#[derive(Debug)]
pub enum TotpError {
    B64(base64::DecodeError),
    Hex(hex::FromHexError),
    Hmac(InvalidKeyLength),
    Time(SystemTimeError),
}

impl fmt::Display for TotpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TotpError::B64(ref err) => write!(f, "Base64 decode error: {}", err),
            TotpError::Hex(ref err) => write!(f, "Hex decode error: {}", err),
            TotpError::Hmac(ref err) => write!(f, "Hmac error: {}", err),
            TotpError::Time(ref err) => write!(f, "System time error: {}", err),
        }
    }
}

impl error::Error for TotpError {
    fn description(&self) -> &str {
        match *self {
            TotpError::B64(ref err) => err.description(),
            TotpError::Hex(ref err) => err.description(),
            TotpError::Hmac(ref err) => err.description(),
            TotpError::Time(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            TotpError::B64(ref err) => Some(err),
            TotpError::Hex(ref err) => Some(err),
            TotpError::Hmac(ref err) => Some(err),
            TotpError::Time(ref err) => Some(err),
        }
    }
}

impl From<base64::DecodeError> for TotpError {
    fn from(err: base64::DecodeError) -> TotpError {
        TotpError::B64(err)
    }
}

impl From<hex::FromHexError> for TotpError {
    fn from(err: hex::FromHexError) -> TotpError {
        TotpError::Hex(err)
    }
}

impl From<InvalidKeyLength> for TotpError {
    fn from(err: InvalidKeyLength) -> TotpError {
        TotpError::Hmac(err)
    }
}

impl From<SystemTimeError> for TotpError {
    fn from(err: SystemTimeError) -> TotpError {
        TotpError::Time(err)
    }
}
