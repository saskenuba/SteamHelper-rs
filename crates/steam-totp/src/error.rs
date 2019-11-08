//! Module containing error types used by this crate.

use super::steam_api::SteamApiResponse;
use base64;
use hex;
use hmac::crypto_mac::InvalidKeyLength;
use reqwest;
use std::{error, fmt, time::SystemTimeError};

/// This error type deals with unresolvable issues coming from the Steam API
/// itself
#[derive(Debug)]
pub enum SteamApiError {
    BadStatusCode(reqwest::Response),
    ParseServerTime(SteamApiResponse),
}

impl fmt::Display for SteamApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SteamApiError::BadStatusCode(ref res) => {
                write!(f, "Received {} status code from Steam API", res.status().as_str())
            }
            SteamApiError::ParseServerTime(ref res) => write!(
                f,
                "Could not parse server_time from Steam response: {:?}",
                res.response.server_time
            ),
        }
    }
}

impl error::Error for SteamApiError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

/// The error type for TOTP operations that wraps underlying errors.
#[derive(Debug)]
pub enum TotpError {
    B64(base64::DecodeError),
    Hex(hex::FromHexError),
    Hmac(InvalidKeyLength),
    Req(reqwest::Error),
    SteamApi(SteamApiError),
    Time(SystemTimeError),
}

impl fmt::Display for TotpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TotpError::B64(ref err) => write!(f, "Base64 decode error: {}", err),
            TotpError::Hex(ref err) => write!(f, "Hex decode error: {}", err),
            TotpError::Hmac(ref err) => write!(f, "Hmac error: {}", err),
            TotpError::Req(ref err) => write!(f, "Request error: {}", err),
            TotpError::SteamApi(ref err) => write!(f, "API error: {}", err),
            TotpError::Time(ref err) => write!(f, "System time error: {}", err),
        }
    }
}

impl error::Error for TotpError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            TotpError::B64(ref err) => Some(err),
            TotpError::Hex(ref err) => Some(err),
            TotpError::Hmac(ref err) => Some(err),
            TotpError::Req(ref err) => Some(err),
            TotpError::SteamApi(ref err) => Some(err),
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

impl From<reqwest::Error> for TotpError {
    fn from(err: reqwest::Error) -> TotpError {
        TotpError::Req(err)
    }
}

impl From<SteamApiError> for TotpError {
    fn from(err: SteamApiError) -> TotpError {
        TotpError::SteamApi(err)
    }
}

impl From<SystemTimeError> for TotpError {
    fn from(err: SystemTimeError) -> TotpError {
        TotpError::Time(err)
    }
}
