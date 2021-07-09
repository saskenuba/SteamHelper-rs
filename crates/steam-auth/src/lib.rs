//! A port of the famous C# SteamAuth library, that allows users to add/remove a mobile
//! authenticator, and also confirm/deny mobile confirmations.

#![allow(dead_code)]
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
use std::fmt::{Debug, Display, Formatter};
use std::fs::OpenOptions;
use std::io::Read;
use std::path::PathBuf;

use const_format::concatcp;
/// re-export
pub use reqwest::{header::HeaderMap, Error as HttpError, Method, Url};
use serde::{Deserialize, Serialize};
use steam_totp::Secret;
use steamid_parser::SteamID;
pub use utils::format_captcha_url;
use uuid::Uuid;

pub use web_handler::steam_guard_linker::AddAuthenticatorStep;
pub use web_handler::confirmation::{ConfirmationMethod, Confirmations, EConfirmationType};

pub mod client;
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
pub const STEAM_COMMUNITY_HOST: &str = ".steamcommunity.com";
/// Steam Help Cookie Host
pub const STEAM_HELP_HOST: &str = ".help.steampowered.com";
/// Steam Store Cookie Host
pub const STEAM_STORE_HOST: &str = ".store.steampowered.com";

/// Should not be used for cookie retrieval. Use `STEAM_COMMUNTY_HOST` instead.
const STEAM_COMMUNITY_BASE: &str = "https://steamcommunity.com";
/// Should not be used for cookie retrieval. Use `STEAM_STORE_HOST` instead.
const STEAM_STORE_BASE: &str = "https://store.steampowered.com";
/// Should not be used for cookie retrieval. Use `STEAM_API_HOST` instead.
const STEAM_API_BASE: &str = "https://api.steampowered.com";

const MOBILE_REFERER: &str = concatcp!(
    STEAM_COMMUNITY_BASE,
    "/mobilelogin?oauth_client_id=DE45CD61&oauth_scope=read_profile%20write_profile%20read_client%20write_client"
);

#[derive(Debug, Clone)]
/// User that is needed for the authenticator to work.
/// Ideally all fields should be populated before authenticator operations are made.
///
/// A simple implementation that has everything required to work properly:
/// ```no_run
/// use steam_auth::User;
///
/// User::build()
///     .username("test_username")
///     .password("password")
///     .parental_code("1111") // Only needed if the is a parental code, otherwise skip
///     .ma_file_from_disk("assets/my.maFile");
/// ```
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
struct CachedInfo {
    steamid: Option<SteamID>,
    api_key: Option<String>,
    /// Oauth token recovered at the login.
    /// Some places call this access_token.
    oauth_token: Option<String>,
}

impl CachedInfo {
    // FIXME: This should not unwrap, probably result with steamid parse error.
    fn set_steamid(&mut self, steamid: &str) {
        let parsed_steamid = SteamID::parse(steamid).unwrap();
        self.steamid = Some(parsed_steamid);
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
    /// Constructs a new user.
    // TODO: This should be a UserBuilder, not simply this methods.
    pub fn build() -> Self {
        Self {
            username: "".to_string(),
            password: "".to_string(),
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
        Some(&self.linked_mafile.as_ref()?.device_id.as_ref()?)
    }

    /// Sets the account username, mandatory
    pub fn username<T: ToString>(mut self, username: T) -> Self {
        self.username = username.to_string();
        self
    }

    /// Sets the account password, mandatory
    pub fn password<T: ToString>(mut self, password: T) -> Self {
        self.password = password.to_string();
        self
    }

    /// Sets the parental code, if any.
    pub fn parental_code<T: ToString>(mut self, parental_code: T) -> Self {
        self.parental_code = Some(parental_code.to_string());
        self
    }

    /// Convenience function that imports the file from disk
    pub fn ma_file_from_disk<T>(mut self, path: T) -> Self
    where
        T: Into<PathBuf>,
    {
        let mut file = OpenOptions::new().read(true).open(path.into()).unwrap();
        let mut buffer = String::new();

        file.read_to_string(&mut buffer).unwrap();
        self.linked_mafile = Some(serde_json::from_str::<MobileAuthFile>(&buffer).unwrap());
        self
    }

    pub fn ma_file(mut self, ma_file: MobileAuthFile) -> Self {
        self.linked_mafile = Some(ma_file);
        self
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
/// The MobileAuthFile (.maFile) is the standard file format that custom authenticators use to save auth secrets to
/// disk.
///
/// It follows strictly the JSON format.
/// Both identity_secret and shared_secret should be base64 encoded. If you don't know if they are, they probably
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

impl Display for MobileAuthFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}

impl MobileAuthFile {
    fn set_device_id(&mut self, device_id: String) {
        self.device_id = Some(device_id)
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
        Self {
            0: Self::PREFIX.to_owned() + &Uuid::new_v4().to_string(),
        }
    }
    pub fn validate() {}
}
