use std::{cell::RefMut, collections::HashMap, time::Duration};

use chrono::Offset;
use cookie::Cookie;
use rand::thread_rng;
use reqwest::{Client, Method};
use rsa::{BigUint, PublicKey, RSAPublicKey};

use const_concat::const_concat;
use steam_totp::{Secret, Time};

use crate::{
    client::MobileClient,
    errors::LoginError,
    types::{LoginRequest, LoginResponseMobile, RSAResponse},
    CachedInfo, User, MOBILE_REFERER, STEAM_COMMUNITY_BASE, STEAM_COMMUNITY_HOST, STEAM_DELAY_MS,
    STEAM_HELP_HOST, STEAM_STORE_HOST,
};

const LOGIN_GETRSA_URL: &str = const_concat!(STEAM_COMMUNITY_BASE, "/login/getrsakey");
const LOGIN_DO_URL: &str = const_concat!(STEAM_COMMUNITY_BASE, "/login/dologin");

type LoginResult<T> = Result<T, LoginError>;

/// This method is used to login through Steam ISteamAuthUser interface.
///
/// Webapi_nonce is received by connecting to the Steam Network.
///
/// Currently not possible without the implementation of the [steam-api] crate.
/// For website that currently works, check [login_website] method.
async fn login_isteam_user_auth(
    _client: &Client,
    _user: User,
    _webapi_nonce: &[u8],
) -> LoginResult<()> {
    let _session_key = steam_crypto::generate_session_key(None).unwrap();

    unimplemented!();
}

/// Website has some quirks to login. Here we handle it.
fn website_handle_rsa(user: &User, response: RSAResponse) -> String {
    let password_bytes = user.password.as_bytes();
    let modulus = hex::decode(response.modulus).unwrap();
    let exponent = hex::decode(response.exponent).unwrap();

    let rsa_encryptor = RSAPublicKey::new(
        BigUint::from_bytes_be(&*modulus),
        BigUint::from_bytes_be(&*exponent),
    )
    .unwrap();
    let mut random_gen = thread_rng();
    let encrypted_pwd_bytes = rsa_encryptor
        .encrypt(
            &mut random_gen,
            rsa::padding::PaddingScheme::PKCS1v15,
            password_bytes,
        )
        .unwrap();

    base64::encode(encrypted_pwd_bytes)
}

/// Used to login through the Steam Website.
/// Also caches the user steamid.
///
///
/// There is also the method that logs in through an API interface called ISteamUserAuth.
/// Check [login_isteam_user_auth]
///
/// https://github.com/Jessecar96/SteamBot/blob/e8e9e5fcd64ae35b201e2597068849c10a667b60/SteamTrade/SteamWeb.cs#L325
// We can really do that method yet, because connection to the SteamNetwork is not yet implemented
// by steam-api crate, and consequently we can't get the user webapi_nonce beforehand.
//
// Should accept closure to handle cases such as needing a captcha or sms.
// But the best way is to have it already setup to use TOTP codes.
pub(crate) async fn login_website(
    client: &MobileClient,
    user: &User,
    mut cached_data: RefMut<'_, CachedInfo>,
) -> LoginResult<()> {
    // we request to generate sessionID cookies
    let response = client
        .request(MOBILE_REFERER.to_owned(), Method::GET, None, None::<&str>)
        .await?;
    let session_id = response
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .map(|cookie| cookie.to_str().unwrap())
        .and_then(|c| {
            let index = c.find('=').unwrap();
            Some((&c[index + 1..index + 25]).to_string())
        })
        .unwrap();

    let mut post_data = HashMap::new();
    let steam_time_offset = (steam_totp::time::Time::offset().await.unwrap() * 1000).to_string();
    post_data.insert("donotcache", &steam_time_offset);
    post_data.insert("username", &user.username);

    let rsa_response = client
        .request(
            LOGIN_GETRSA_URL.to_owned(),
            Method::POST,
            None,
            Some(post_data.clone()),
        )
        .await?;

    // wait for steam to catch up
    tokio::time::delay_for(Duration::from_millis(STEAM_DELAY_MS)).await;

    // rsa handling
    let response = rsa_response.json::<RSAResponse>().await.unwrap();
    let encrypted_pwd_b64 = website_handle_rsa(&user, response.clone());

    let offset = Time::offset().await.unwrap();
    let time = Time::now(Some(offset)).unwrap();

    let steam_time_offset = (offset * 1000).to_string();
    let two_factor_code = user
        .linked_mafile
        .as_ref()
        .map(|f| Secret::from_b64(&f.shared_secret).unwrap())
        .map(|s| steam_totp::generate_auth_code(s, time))
        .unwrap_or_else(|| "".to_string());

    let login_request = LoginRequest {
        donotcache: &steam_time_offset,
        password: &encrypted_pwd_b64,
        username: &user.username,
        twofactorcode: &two_factor_code,
        emailauth: "",
        captcha_gid: "-1",
        captcha_text: "",
        emailsteamid: "",
        rsa_timestamp: response.timestamp,
        ..Default::default()
    };

    let login_response = client
        .request(
            LOGIN_DO_URL.to_owned(),
            Method::POST,
            None,
            Some(login_request),
        )
        .await?;

    let login_response_json = login_response
        .json::<LoginResponseMobile>()
        .await
        .map_err(|_| {
            LoginError::GeneralFailure(
                "Non mobile response detected. Contact maintainer.".to_string(),
            )
        })?;

    let steamid = login_response_json.oauth.steamid;
    let token = login_response_json.oauth.wgtoken;
    let token_secure = login_response_json.oauth.wgtoken_secure;

    // cache steamid
    cached_data.set_steamid(&steamid);

    {
        // Recover cookies to authorize store.steampowered and help.steampowered subdomains.
        let mut cookie_jar = client.cookie_store.borrow_mut();
        vec![STEAM_COMMUNITY_HOST, STEAM_HELP_HOST, STEAM_STORE_HOST]
            .into_iter()
            .for_each(|host| {
                let timezone_offset = format!(
                    "{},0",
                    chrono::Local::now().offset().fix().local_minus_utc()
                );
                let fmt_token = format!("{}%7C%7C{}", steamid, token);
                let fmt_secure_token = format!("{}%7C%7C{}", steamid, token_secure);
                cookie_jar.add_original(
                    Cookie::build("steamLoginSecure", fmt_secure_token)
                        .domain(host)
                        .path("/")
                        .finish(),
                );
                cookie_jar.add_original(
                    Cookie::build("steamLogin", fmt_token)
                        .domain(host)
                        .path("/")
                        .finish(),
                );
                cookie_jar.add_original(
                    Cookie::build("sessionid", session_id.clone())
                        .domain(host)
                        .path("/")
                        .finish(),
                );
                cookie_jar.add_original(
                    Cookie::build("timezoneOffset", timezone_offset)
                        .domain(host)
                        .path("/")
                        .finish(),
                );
            });
    }

    Ok(())
}
