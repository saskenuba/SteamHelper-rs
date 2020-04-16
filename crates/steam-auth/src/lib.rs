#![allow(dead_code)]
#![feature(str_strip)]

use std::{fs::OpenOptions, io::Read};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use const_concat::const_concat;
use steam_totp::Secret;

mod client;
mod enums;
mod errors;
mod page_scraper;
mod types;
mod utils;
mod web_handler;

/// Recommended time to allow STEAM to catch up.
const STEAM_DELAY_MS: u64 = 350;
/// Extension of the mobile authenticator files.
const MA_FILE_EXT: &str = ".maFile";

const STEAM_COMMUNITY_HOST: &str = ".steamcommunity.com";
const STEAM_HELP_HOST: &str = ".help.steampowered.com";
const STEAM_STORE_HOST: &str = ".store.steampowered.com";

const STEAM_COMMUNITY_BASE: &str = "https://steamcommunity.com";
const STEAM_STORE_BASE: &str = "https://store.steampowered.com";
const STEAM_API_BASE: &str = "https://api.steampowered.com";

const MOBILE_REFERER: &str = const_concat!(
    STEAM_COMMUNITY_BASE,
    "/mobilelogin?oauth_client_id=DE45CD61&oauth_scope=read_profile%20write_profile%20read_client\
    %20write_client"
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    username: String,
    password: String,
    parental_code: Option<String>,
    linked_mafile: Option<MobileAuthFile>,
    cached_info: CachedInfo,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
/// After login, we cache some information from the user so there is no need to keep manually
/// querying Steam multiple times.
struct CachedInfo {
    pub steamid: Option<String>,
    pub api_key: Option<String>,
}

impl User {
    fn build() -> Self {
        Self {
            username: "".to_string(),
            password: "".to_string(),
            parental_code: None,
            linked_mafile: None,
            cached_info: Default::default(),
        }
    }

    fn shared_secret(&self) -> Option<Secret>{
        Some(Secret::from_b64(&self.linked_mafile.as_ref()?.shared_secret).unwrap())
    }

    fn identity_secret(&self) -> Option<Secret>{
        Some(Secret::from_b64(&self.linked_mafile.as_ref()?.identity_secret).unwrap())
    }

    fn device_id(&self) -> Option<&str>{
        Some(&self.linked_mafile.as_ref()?.device_id.as_ref()?)
    }

    fn steam_id(&self) -> Option<&str> {
        Some(&self.cached_info.steamid.as_ref()?)
    }

    fn username<T: ToString>(mut self, username: T) -> Self {
        self.username = username.to_string();
        self
    }

    fn password<T: ToString>(mut self, password: T) -> Self {
        self.password = password.to_string();
        self
    }

    fn parental_code<T: ToString>(mut self, parental_code: T) -> Self {
        self.parental_code = Some(parental_code.to_string());
        self
    }

    /// Convenience function that imports the file from disk
    fn ma_file_from_disk(mut self, path: &str) -> Self {
        let mut file = OpenOptions::new().read(true).open(path).unwrap();
        let mut buffer = String::new();

        file.read_to_string(&mut buffer).unwrap();
        self.linked_mafile = Some(serde_json::from_str::<MobileAuthFile>(&buffer).unwrap());
        self
    }

    fn ma_file_from_string(mut self, ma_file: &str) -> Self {
        self.linked_mafile = Some(MobileAuthFile::from(ma_file));
        self
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
/// The MobileAuthFile (.maFile) is the standard that some custom authenticators use to
/// save the auth secrets to disk. It follows the json format.
struct MobileAuthFile {
    /// Identity secret is used to generate the confirmation links for our trade requests.
    /// If we are generating our own Authenticator, this is given by Steam.
    identity_secret: String,
    /// The shared secret is used to generate TOTP codes.
    shared_secret: String,
    /// Device ID is used to generate the confirmation links for our trade requests.
    /// Can be retrieved from mobile device, such as a rooted android, iOS, or generated randomly if
    /// creating our own authenticator.
    /// Needed for confirmations to trade to work properly.
    device_id: Option<String>,
    /// Used if shared secret is lost. Please, don't lose it.
    recovery_code: Option<String>,
}

impl From<&str> for MobileAuthFile {
    fn from(string: &str) -> Self {
        serde_json::from_str(string).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// Identifies the mobile device and needed to generate confirmation links.
/// It is on the format of a UUID V4.
struct DeviceId(String);

impl DeviceId {
    const PREFIX: &'static str = "android:";

    /// Generates a random device ID on the format of UUID v4
    /// Example: android:780c3700-2b4f-4b9a-a196-9af6e6010d09
    pub fn generate() -> Self {
        Self { 0: Self::PREFIX.to_owned() + &Uuid::new_v4().to_string() }
    }
    pub fn validate() {}
}
