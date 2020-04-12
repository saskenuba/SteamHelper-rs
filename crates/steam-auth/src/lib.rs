#![allow(dead_code)]
#![feature(str_strip)]

use std::{fs::OpenOptions, io::Read};

use cookie::{Cookie, CookieJar};
use reqwest::{header::HeaderMap, Client, Method, Response, Url};
use scraper::Html;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use const_concat::const_concat;

use crate::utils::dump_cookies_by_domain;
use std::cell::RefCell;

mod errors;
mod steam_scraper;
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
const STEAM_API_BASE: &str = "https://api.steampowered.com";

/// used to refresh session
const MOBILE_AUTH_GETWGTOKEN: &str =
    const_concat!(STEAM_API_BASE, "/IMobileAuthService/GetWGToken/v0001");

const MOBILE_REFERER: &str = const_concat!(
    STEAM_COMMUNITY_BASE,
    "/mobilelogin?oauth_client_id=DE45CD61&oauth_scope=read_profile%20write_profile%20read_client\
    %20write_client"
);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    pub username: String,
    pub password: String,
    pub parental_code: Option<String>,
    pub linked_mafile: Option<MobileAuthFile>,
    pub cached_info: CachedInfo,
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

#[derive(Debug)]
/// Main authenticator. We use it to spawn and act as our "mobile" client.
/// Responsible for accepting/denying trades, and some other operations
/// that may or not be related to only mobile operations.
struct SteamAuthenticator {
    pub client: MobileClient,
    pub user: User,
}

#[derive(Debug)]
struct MobileClient {
    /// Standard HTTP Client to make requests.
    pub client: reqwest::Client,
    /// We manually handle cookies, because reqwest is just not good at it.
    pub cookie_store: RefCell<CookieJar>,
}

impl MobileClient {
    async fn request_ensure_login<T: Serialize>(
        &self,
        url: &str,
        method: reqwest::Method,
        custom_headers: Option<HeaderMap>,
        data: Option<T>,
    ) {
        unimplemented!()
    }

    // remember that we need to disallow redirects
    // because it may redirect us to weird paths, such as steamobile://xxx
    async fn request<T: Serialize>(
        &self,
        url: &str,
        method: reqwest::Method,
        custom_headers: Option<HeaderMap>,
        data: Option<T>,
    ) -> Result<Response, reqwest::Error> {
        let parsed_url = Url::parse(url).unwrap();
        let mut header_map = custom_headers.unwrap_or_default();

        // Send cookies from jar, based on domain
        let domain = &format!(".{}", parsed_url.host_str().unwrap());
        let domain_cookies = dump_cookies_by_domain(
            &self.cookie_store.borrow(),
            domain,
        );
        header_map.insert(
            reqwest::header::COOKIE,
            domain_cookies.unwrap_or_else(|| "".to_string()).parse().unwrap(),
        );

        let req_builder = self.client.request(method, parsed_url).headers(header_map);

        let request = match data {
            None => req_builder.build().unwrap(),
            Some(data) => req_builder.form(&data).build().unwrap(),
        };

        // dbg!(&request);
        Ok(self.client.execute(request).await?)
    }

    /// Convenience function to retrieve HTML
    async fn get_html(&self, url: &str) -> Result<Html, reqwest::Error> {
        let response = self.request(url, Method::GET, None, None::<&str>).await?;
        let response_text = response.text().await?;
        let _html_document = Html::parse_document(&response_text);

        Ok(_html_document)
    }

    /// Initiate cookie jar with mobile cookies
    fn init_cookie_jar() -> CookieJar {
        let mut mobile_cookies = CookieJar::new();
        mobile_cookies.add(Cookie::build("Steam_Language", "english").domain(STEAM_COMMUNITY_HOST).finish());
        mobile_cookies.add(Cookie::build("mobileClient", "android").domain(STEAM_COMMUNITY_HOST).finish());
        mobile_cookies.add(Cookie::build("mobileClientVersion", "0 (2.1.3)").domain(STEAM_COMMUNITY_HOST).finish());
        mobile_cookies
    }

    /// Initiate mobile client with default headers
    fn init_mobile_client() -> Client {
        let user_agent = "Mozilla/5.0 (Linux; U; Android 4.1.1; en-us; Google Nexus 4 - 4.1.1 - \
    API 16 - 768x1280 Build/JRO03S) AppleWebKit/534.30 (KHTML, like Gecko) Version/4.0 Mobile Safari/534.30";
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            reqwest::header::ACCEPT,
            "text/javascript, text/html, application/xml, text/xml, */*".parse().unwrap(),
        );
        default_headers.insert(reqwest::header::REFERER, MOBILE_REFERER.parse().unwrap());

        default_headers.insert(
            "X-Requested-With",
            "com.valvesoftware.android.steam.community".parse().unwrap(),
        );

        reqwest::Client::builder()
            .cookie_store(true)
            .user_agent(user_agent)
            .default_headers(default_headers)
            .referer(false)
            .build()
            .unwrap()
    }
}

impl Default for MobileClient {
    fn default() -> Self {
        Self { client: Self::init_mobile_client(), cookie_store: RefCell::new(Self::init_cookie_jar()) }
    }
}

/// Generate a Steam Authenticator that you use to login into the Steam Community / Steam Store.
impl SteamAuthenticator {
    fn new(user: User) -> Self {
        Self { client: MobileClient::default(), user }
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