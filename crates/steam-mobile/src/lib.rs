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

pub use client::SteamAuthenticator;
use const_format::concatcp;
pub use reqwest::header::HeaderMap;
pub use reqwest::Method;
pub use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use steam_totp::Secret;
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
pub(crate) mod utils;
mod web_handler;

/// Recommended time to allow STEAM to catch up.
const STEAM_DELAY_MS: u64 = 350;
/// Extension of the mobile authenticator files.
const MA_FILE_EXT: &str = ".maFile";

// HOST SHOULD BE USED FOR COOKIE RETRIEVAL INSIDE COOKIE JAR!!

/// Steam Community Cookie Host
pub(crate) const STEAM_COMMUNITY_HOST: &str = ".steamcommunity.com";
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

/// User that is needed for the authenticator to work.
/// Ideally all fields should be populated before authenticator operations are made.
///
/// A simple implementation that has everything required to work properly:
/// ```no_run
/// use steam_mobile::User;
///
/// User::new("test_username".to_string(), "password".to_string())
///     .parental_code("1111") // Only needed if the is a parental code, otherwise skip
///     .ma_file_from_disk("assets/my.maFile");
/// ```
#[derive(Debug, Clone)]
pub struct User {
    username: String,
    password: String,
    parental_code: Option<String>,
    linked_mafile: Option<MobileAuthFile>,
}

#[derive(Default, Debug, Clone)]
/// Information that we cache after the login operation to avoid querying Steam multiple times.
///
///
/// SteamID, API KEY and the login Oauth token are currently cached by `SteamAuthenticator`.
struct SteamCache {
    steamid: Option<SteamID>,
    api_key: Option<String>,
    /// Oauth token recovered at the login.
    /// Some places call this access_token.
    oauth_token: Option<String>,
}

impl SteamCache {
    fn set_steamid(&mut self, steamid: &str) -> Result<(), InternalError> {
        let parsed_steamid = SteamID::parse(steamid).ok_or_else(|| {
            let err_str = format!("Failed to parse {steamid} as SteamID.");
            InternalError::GeneralFailure(err_str)
        })?;
        self.steamid = Some(parsed_steamid);
        Ok(())
    }

    fn set_oauth_token(&mut self, token: String) {
        self.oauth_token = Some(token);
    }

    fn set_api_key(&mut self, api_key: String) {
        self.api_key = Some(api_key);
    }

    fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    fn steam_id(&self) -> Option<u64> {
        Some(self.steamid.as_ref()?.to_steam64())
    }

    fn oauth_token(&self) -> Option<&str> {
        self.oauth_token.as_deref()
    }
}

impl User {
    /// Creates a new valid `User` with the bare minimum credentials.
    #[must_use]
    pub fn new(username: String, password: String) -> Self {
        Self {
            username,
            password,
            parental_code: None,
            linked_mafile: None,
        }
    }

    fn shared_secret(&self) -> Option<Secret> {
        Some(Secret::from_b64(&self.linked_mafile.as_ref()?.shared_secret).unwrap())
    }

    fn identity_secret(&self) -> Option<Secret> {
        Some(Secret::from_b64(&self.linked_mafile.as_ref()?.identity_secret).unwrap())
    }

    fn device_id(&self) -> Option<&str> {
        Some(self.linked_mafile.as_ref()?.device_id.as_ref()?)
    }

    /// Sets the account username, mandatory
    #[must_use]
    pub fn username<T: ToString>(mut self, username: T) -> Self {
        self.username = username.to_string();
        self
    }

    /// Sets the account password, mandatory
    #[must_use]
    pub fn password<T: ToString>(mut self, password: T) -> Self {
        self.password = password.to_string();
        self
    }

    /// Sets the parental code, if any.
    #[must_use]
    pub fn parental_code<T: ToString>(mut self, parental_code: T) -> Self {
        self.parental_code = Some(parental_code.to_string());
        self
    }

    /// Convenience function that imports the file from disk
    pub fn ma_file_from_disk<T>(mut self, path: T) -> Result<Self, AuthError>
    where
        T: Into<PathBuf>,
    {
        self.linked_mafile = Some(MobileAuthFile::from_disk(path)?);
        Ok(self)
    }

    #[allow(missing_docs)]
    #[must_use]
    pub fn ma_file(mut self, ma_file: MobileAuthFile) -> Self {
        self.linked_mafile = Some(ma_file);
        self
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
