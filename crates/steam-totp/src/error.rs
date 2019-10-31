use hmac::crypto_mac::InvalidKeyLength;
use std::{
    error,
    fmt,
    time::SystemTimeError,
};

/// The error type for TOTP operations that wraps underlying errors.
#[derive(Debug)]
pub enum TotpError {
    Time(SystemTimeError),
    Hmac(InvalidKeyLength),
}

impl fmt::Display for TotpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TotpError::Time(ref err) => write!(f, "SystemTime error: {}", err),
            TotpError::Hmac(ref err) => write!(f, "Hmac error: {}", err),
        }
    }
}

impl error::Error for TotpError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            TotpError::Time(ref err) => Some(err),
            TotpError::Hmac(ref err) => Some(err),
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
