//! A port of the famous C# SteamAuth library, that allows users to add/remove a mobile
//! authenticator, and also confirm/deny mobile confirmations.

#![allow(dead_code, clippy::missing_errors_doc)]
#![warn(missing_docs, missing_doc_code_examples)]
#![deny(
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_import_braces,
    unused_qualifications
)]

use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::path::PathBuf;
use std::sync::Arc;

pub use client::Authenticated;
pub use client::SteamAuthenticator;
pub use client::Unauthenticated;
use const_format::concatcp;
use parking_lot::RwLock;
pub use reqwest::header::HeaderMap;
pub use reqwest::Method;
pub use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use steamid_parser::SteamID;
pub use utils::format_captcha_url;
use uuid::Uuid;
pub use web_handler::confirmation::ConfirmationMethod;
pub use web_handler::confirmation::Confirmations;
pub use web_handler::confirmation::EConfirmationType;
pub use web_handler::steam_guard_linker::AddAuthenticatorStep;

use crate::errors::AuthError;
use crate::errors::InternalError;
use crate::errors::MobileAuthFileError;
use crate::utils::read_from_disk;

mod adapter;
pub(crate) mod client;
pub mod errors;
mod page_scraper;
pub(crate) mod retry;
mod types;
pub mod user;
pub(crate) mod utils;
mod web_handler;

/// Recommended time to allow STEAM to catch up.
const STEAM_DELAY_MS: u64 = 350;
/// Extension of the mobile authenticator files.
const MA_FILE_EXT: &str = ".maFile";

// HOST SHOULD BE USED FOR COOKIE RETRIEVAL INSIDE COOKIE JAR!!

/// Steam Community Cookie Host
pub(crate) const STEAM_COMMUNITY_HOST: &str = "steamcommunity.com";
/// Steam Help Cookie Host
pub(crate) const STEAM_HELP_HOST: &str = ".help.steampowered.com";
/// Steam Store Cookie Host
pub(crate) const STEAM_STORE_HOST: &str = ".store.steampowered.com";

/// Should not be used for cookie retrieval. Use `STEAM_COMMUNTY_HOST` instead.
pub(crate) const STEAM_COMMUNITY_BASE: &str = "https://steamcommunity.com";
/// Should not be used for cookie retrieval. Use `STEAM_STORE_HOST` instead.
pub(crate) const STEAM_STORE_BASE: &str = "https://store.steampowered.com";
/// Should not be used for cookie retrieval. Use `STEAM_API_HOST` instead.
pub(crate) const STEAM_API_BASE: &str = "https://api.steampowered.com";

pub(crate) const STEAM_LOGIN_BASE: &str = "https://login.steampowered.com";

const MOBILE_REFERER: &str = concatcp!(
    STEAM_COMMUNITY_BASE,
    "/mobilelogin?oauth_client_id=DE45CD61&oauth_scope=read_profile%20write_profile%20read_client%20write_client"
);

#[allow(missing_docs)]
pub type AuthResult<T> = Result<T, AuthError>;

/// Information that we cache after the login operation to avoid querying Steam multiple times.
#[derive(Debug, Clone)]
struct SteamCache {
    steamid: SteamID,
    api_key: Option<String>,
    /// Oauth token recovered at the login.
    oauth_token: String,
    access_token: String,
}

pub(crate) type CacheGuard = Arc<RwLock<SteamCache>>;

impl SteamCache {
    fn query_tokens(&self) -> Vec<(&'static str, String)> {
        [("access_token", self.access_token.clone())].to_vec()
    }

    fn with_login_data(steamid: &str, access_token: String, refresh_token: String) -> Result<Self, InternalError> {
        let parsed_steamid = SteamID::parse(steamid).ok_or_else(|| {
            let err_str = format!("Failed to parse {steamid} as SteamID.");
            InternalError::GeneralFailure(err_str)
        })?;

        Ok(Self {
            steamid: parsed_steamid,
            api_key: None,
            oauth_token: refresh_token,
            access_token,
        })
    }

    fn set_api_key(&mut self, api_key: Option<String>) {
        self.api_key = api_key;
    }

    fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    fn steam_id(&self) -> u64 {
        self.steamid.to_steam64()
    }

    fn oauth_token(&self) -> &str {
        &self.oauth_token
    }
}

/// The `MobileAuthFile` (.maFile) is the standard file format that custom authenticators use to save auth secrets to
/// disk.
///
/// It follows strictly the JSON format.
/// Both `identity_secret` and `shared_secret` should be base64 encoded. If you don't know if they are, they probably
/// already are.
///
///
/// Example:
/// ```json
/// {
///     identity_secret: "secret"
///     shared_secret: "secret"
///     device_id: "android:xxxxxxxxxxxxxxx"
/// }
/// ```
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MobileAuthFile {
    /// Identity secret is used to generate the confirmation links for our trade requests.
    /// If we are generating our own Authenticator, this is given by Steam.
    identity_secret: String,
    /// The shared secret is used to generate TOTP codes.
    shared_secret: String,
    /// Device ID is used to generate the confirmation links for our trade requests.
    /// Can be retrieved from mobile device, such as a rooted android, iOS, or generated from the account's SteamID if
    /// creating our own authenticator. Needed for confirmations to trade to work properly.
    device_id: Option<String>,
    /// Used if shared secret is lost. Please, don't lose it.
    revocation_code: Option<String>,
    /// Account name where this maFile was originated.
    pub account_name: Option<String>,
}

impl Debug for MobileAuthFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MobileAuthFile")
            .field("AccountName", &self.account_name)
            .finish()
    }
}

impl MobileAuthFile {
    fn set_device_id(&mut self, device_id: String) {
        self.device_id = Some(device_id);
    }

    /// Creates a new `MobileAuthFile`
    pub fn new<T>(identity_secret: String, shared_secret: String, device_id: T) -> Self
    where
        T: Into<Option<String>>,
    {
        Self {
            identity_secret,
            shared_secret,
            device_id: device_id.into(),
            revocation_code: None,
            account_name: None,
        }
    }

    /// Parses a [`MobileAuthFile`] from a json string.
    pub fn from_json(string: &str) -> Result<Self, MobileAuthFileError> {
        serde_json::from_str::<Self>(string)
            .map_err(|e| MobileAuthFileError::InternalError(InternalError::DeserializationError(e)))
    }

    /// Convenience function that imports the file from disk
    ///
    /// # Panic
    /// Will panic if file is not found.
    pub fn from_disk<T>(path: T) -> Result<Self, MobileAuthFileError>
    where
        T: Into<PathBuf>,
    {
        let buffer = read_from_disk(path);
        Self::from_json(&buffer)
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// Identifies the mobile device and needed to generate confirmation links.
///
/// It is on the format of a UUID V4.
struct DeviceId(String);

impl DeviceId {
    const PREFIX: &'static str = "android:";

    /// Generates a random device ID on the format of UUID v4
    /// Example: android:780c3700-2b4f-4b9a-a196-9af6e6010d09
    pub fn generate() -> Self {
        Self(Self::PREFIX.to_owned() + &Uuid::new_v4().to_string())
    }
    pub fn validate() {}
}
