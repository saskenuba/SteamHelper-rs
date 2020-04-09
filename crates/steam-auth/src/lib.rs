#![allow(dead_code)]
#![feature(async_closure)]

use std::{collections::HashMap, fs::OpenOptions, io::Read, time::Duration};

use rand::thread_rng;
use reqwest::{header::HeaderMap, Client, Method, Response, Url};
use rsa::{BigUint, PublicKey, RSAPublicKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use const_concat::const_concat;

use crate::responses::{LoginRequest, LoginResponse, RSAResponse};

mod errors;
mod confirmations;
mod responses;
mod scraper;
mod utils;

/// Recommended time to allow STEAM to catch up.
const STEAM_DELAY_MS: u64 = 350;
/// Extension of the mobile authenticator files.
const MOBILEAUTH_EXT: &str = ".maFile";

const STEAMAPI_BASE: &str = "https://api.steampowered.com";
const COMMUNITY_BASE: &str = "https://steamcommunity.com";

const LOGIN_GETRSA_URL: &str = const_concat!(COMMUNITY_BASE, "/login/getrsakey");
const LOGIN_DO_URL: &str = const_concat!(COMMUNITY_BASE, "/login/dologin");

const MOBILEAUTH_GETWGTOKEN: &str =
    const_concat!(STEAMAPI_BASE, "/IMobileAuthService/GetWGToken/v0001");

const TWO_FACTOR_TIME_QUERY: &str =
    const_concat!(STEAMAPI_BASE, "/ITwoFactorService/QueryTime/v0001");

const MOBILE_REFERER: &str = const_concat!(
    COMMUNITY_BASE,
    "/mobilelogin?oauth_client_id=DE45CD61&oauth_scope=read_profile%20write_profile%20read_client%20write_client"
);

#[derive(Serialize, Deserialize)]
struct User {
    pub username: String,
    pub password: String,
}

#[derive(Debug)]
/// Main authenticator. We use it to spawn and act as our "mobile" client.
/// Responsible for accepting/denying trades.
struct SteamAuthenticator {
    pub client: reqwest::Client,
    pub ma_file: Option<MobileAuthFile>,
}

/// Generate a Steam Authenticator that you use to login into the Steam Community / Steam Store.
impl SteamAuthenticator {
    fn build() -> Self {
        Self { client: create_mobile_client(), ma_file: None }
    }

    /// Convenience function that imports the file from disk
    fn ma_file_from_disk(mut self, path: &str) -> SteamAuthenticator {
        let mut file = OpenOptions::new().read(true).open(path).unwrap();
        let mut buffer = String::new();

        file.read_to_string(&mut buffer).unwrap();
        self.ma_file = Some(serde_json::from_str::<MobileAuthFile>(&buffer).unwrap());
        self
    }

    fn ma_file_from_string(mut self, ma_file: &str) -> SteamAuthenticator {
        self.ma_file = Some(serde_json::from_str::<MobileAuthFile>(ma_file).unwrap());
        self
    }

    fn finish(mut self) -> SteamAuthenticator {
        self
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// The MobileAuthFile (.maFile) is the standard that some custom authenticators use to
/// save the auth secrets to disk. It follows the json format.
struct MobileAuthFile {
    /// Identity secret is used to generate the confirmation links for our trade requests.
    /// If we are generating our own Authenticator, this is given by Steam.
    identity_secret: String,
    /// The shared secret is used to generate TOTP codes.
    shared_secret: String,
    /// Device ID is used to generate the confirmation links for our trade requests.
    /// Can be generated randomly.
    device_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
/// Identifies the mobile device and needed to generate confirmation links.
struct DeviceId(String);

impl DeviceId {
    const PREFIX: &'static str = "android:";

    /// Generates a random device ID on the format of 'android:780c3700-2b4f-4b9a-a196-9af6e6010d09'
    pub fn generate() -> Self {
        Self { 0: Self::PREFIX.to_owned() + &Uuid::new_v4().to_string() }
    }
    pub fn validate() {}
}

fn create_mobile_client() -> Client {
    let user_agent = "Mozilla/5.0 (Linux; U; Android 4.1.1; en-us; Google Nexus 4 - 4.1.1 - \
    API 16 - 768x1280 Build/JRO03S) AppleWebKit/534.30 (KHTML, like Gecko) Version/4.0 Mobile Safari/534.30";
    let mut default_headers = HeaderMap::new();
    default_headers.insert(
        reqwest::header::ACCEPT,
        "text/javascript, text/html, application/xml, text/xml, */*".parse().unwrap(),
    );
    default_headers.insert(reqwest::header::REFERER, MOBILE_REFERER.parse().unwrap());

    // steam-auth uses it, so we use it too
    default_headers
        .insert("X-Requested-With", "com.valvesoftware.android.steam.community".parse().unwrap());

    // mobile cookies
    default_headers.insert(
        reqwest::header::COOKIE,
        "Steam_Language=english; mobileClient=android; mobileClientVersion=0 (2.1.3); Path=/; \
        Domain=.steamcommunity.com"
            .parse()
            .unwrap(),
    );

    reqwest::Client::builder()
        .cookie_store(true)
        .user_agent(user_agent)
        .default_headers(default_headers)
        .referer(false)
        .build()
        .unwrap()
}

async fn mobile_request<T: Serialize>(
    client: &Client,
    url: &str,
    method: reqwest::Method,
    custom_headers: Option<HeaderMap>,
    data: Option<T>,
) -> Response {
    let referer = MOBILE_REFERER;
    dbg!(&client);

    let pre_client = client
        .request(method, Url::parse(url).unwrap())
        .headers(custom_headers.unwrap_or_default());

    let request = match data {
        None => pre_client.build().unwrap(),
        Some(data) => pre_client.form(&data).build().unwrap(),
    };

    dbg!(&request);

    client.execute(request).await.unwrap()
}

async fn login(user: User) {
    let client = create_mobile_client();

    // load Session cookies
    mobile_request(&client, MOBILE_REFERER, Method::GET, None, None::<&str>).await;

    let mut post_data = HashMap::new();
    let steam_time_offset = (steam_totp::time::Time::offset().await.unwrap() * 1000).to_string();
    post_data.insert("donotcache", steam_time_offset.as_str());
    post_data.insert("username", &user.username);

    let rsa_response =
        mobile_request(&client, LOGIN_GETRSA_URL, Method::POST, None, Some(post_data.clone()))
            .await;

    let response = rsa_response.json::<RSAResponse>().await.unwrap();
    println!("response: {:?}", response);

    // wait for steam to catch up
    tokio::time::delay_for(Duration::from_millis(STEAM_DELAY_MS)).await;

    // rsa handling
    let password_bytes = user.password.as_bytes();
    let modulus = hex::decode(response.modulus).unwrap();
    let exponent = hex::decode(response.exponent).unwrap();
    let rsa_encryptor =
        RSAPublicKey::new(BigUint::from_bytes_be(&*modulus), BigUint::from_bytes_be(&*exponent))
            .unwrap();

    let mut random_gen = thread_rng();
    let encrypted_pwd_bytes = rsa_encryptor
        .encrypt(&mut random_gen, rsa::padding::PaddingScheme::PKCS1v15, password_bytes)
        .unwrap();
    let encrypted_pwd_b64 = base64::encode(encrypted_pwd_bytes);

    // finish login
    let two_factor_code: Option<&str> = None;
    let require_captcha: Option<&str> = None;
    let require_2fa: Option<&str> = None;
    let require_email: Option<&str> = None;

    post_data.clear();
    let steam_time_offset = (steam_totp::time::Time::offset().await.unwrap() * 1000).to_string();

    let login_request = LoginRequest {
        donotcache: &steam_time_offset,
        password: &encrypted_pwd_b64,
        username: &user.username,
        twofactorcode: two_factor_code.unwrap_or(""),
        emailauth: "".to_string(),
        loginfriendlyname: "".to_string(),
        captchagid: require_captcha.unwrap_or("-1"),
        captcha_text: require_captcha.unwrap_or(""),
        emailsteamid: "".to_string(),
        rsatimestamp: response.timestamp,
        ..Default::default()
    };

    let login_response =
        mobile_request(&client, LOGIN_DO_URL, Method::POST, None, Some(login_request)).await;
    dbg!(&login_response);

    let login_response_json = login_response.json::<LoginResponse>().await;
    println!("Login response: {:?}", login_response_json);
}

#[cfg(test)]
mod tests {
    use crate::{SteamAuthenticator, User};

    #[test]
    fn test_new_authenticator_with_ma_file() {
        SteamAuthenticator::build().ma_file_from_disk("assets/sample.maFile");
    }
}
