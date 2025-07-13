//! Module containing error types used by this crate.

use super::steam_api::SteamApiResponse;
use base64;
use crypto_mac::InvalidKeyLength;
use hex;
use hmac::digest::InvalidLength;
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
#[derive(Debug, thiserror::Error)]
pub enum TotpError {
    #[error("Base64 decode error: {0}")]
    B64(#[from] base64::DecodeError),
    #[error("Hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("HMAC error: {0}")]
    Hmac(#[from] InvalidLength),
    #[error("Request error: {0}")]
    Req(#[from] reqwest::Error),
    #[error("Steam API error: {0}")]
    SteamApi(#[from] SteamApiError),
    #[error("System time error: {0}")]
    Time(#[from] SystemTimeError),
}

