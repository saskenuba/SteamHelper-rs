use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use const_format::concatcp;
use futures_timer::Delay;
use futures_util::stream::FuturesUnordered;
use futures_util::{StreamExt, TryFutureExt};
use login_types::{
    BeginAuthSessionViaCredentialsRequest, BeginAuthSessionViaCredentialsResponseBase,
    UpdateAuthSessionWithSteamGuardCodeRequest,
};
use parking_lot::RwLock;
use rand::thread_rng;
use reqwest::{Client, Method};
use rsa::{BigUint, Pkcs1v15Encrypt, RsaPublicKey};
use serde_json::json;
use steam_totp::{Secret, Time};
use tracing::{debug, error};

use crate::adapter::SteamCookie;
use crate::client::MobileClient;
use crate::errors::LoginError;
use crate::types::{DomainToken, FinalizeLoginRequest, FinalizeLoginResponseBase, RSAResponseBase};
use crate::web_handler::login::login_types::{PollAuthSessionStatusRequest, PollAuthSessionStatusResponseBase};
use crate::{
    AuthResult, SteamCache, User, MOBILE_REFERER, STEAM_API_BASE, STEAM_COMMUNITY_BASE, STEAM_COMMUNITY_HOST,
    STEAM_DELAY_MS, STEAM_HELP_HOST, STEAM_LOGIN_BASE, STEAM_STORE_HOST,
};

mod login_types;

const LOGIN_RSA_ENDPOINT: &str = concatcp!(STEAM_API_BASE, "/IAuthenticationService/GetPasswordRSAPublicKey/v1/");
const LOGIN_BEGIN_AUTH_ENDPOINT: &str = concatcp!(
    STEAM_API_BASE,
    "/IAuthenticationService/BeginAuthSessionViaCredentials/v1/"
);
const LOGIN_UPDATE_STEAM_GUARD_ENDPOINT: &str = concatcp!(
    STEAM_API_BASE,
    "/IAuthenticationService/UpdateAuthSessionWithSteamGuardCode/v1/"
);

const LOGIN_POLL_AUTH_STATUS_ENDPOINT: &str =
    concatcp!(STEAM_API_BASE, "/IAuthenticationService/PollAuthSessionStatus/v1/");

const LOGIN_FINALIZE_LOGIN_ENDPOINT: &str = concatcp!(STEAM_LOGIN_BASE, "/jwt/finalizelogin");

/// This method is used to login through Steam `ISteamAuthUser` interface.
///
/// Webapi_nonce is received by connecting to the Steam Network.
///
/// Currently not possible without the implementation of the [steam-client] crate.
/// For website that currently works, check [login_and_store_cookies] method.
async fn login_isteam_user_auth(_client: &Client, _user: User, _webapi_nonce: &[u8]) -> AuthResult<()> {
    unimplemented!();
}

fn encrypt_password<MOD, EXP>(user: &User, modulus: MOD, exponent: EXP) -> String
where
    MOD: AsRef<[u8]>,
    EXP: AsRef<[u8]>,
{
    let password_bytes = user.password.as_bytes();
    let exponent = BigUint::parse_bytes(exponent.as_ref(), 16).unwrap();
    let modulus = BigUint::parse_bytes(modulus.as_ref(), 16).unwrap();

    let encrypted = RsaPublicKey::new(modulus, exponent)
        .expect("Failed to create public key.")
        .encrypt(&mut thread_rng(), Pkcs1v15Encrypt, password_bytes)
        .expect("Failed to encrypt.");

    base64::engine::general_purpose::STANDARD.encode(encrypted)
}

pub const SESSION_ID_COOKIE: &str = "sessionid";

/// Used to login through the Steam Website.
/// Caches user's steamID.
///
/// Stores sessions cookies for steamcommunity, steampowered.
///
///
/// There is also the method that logs in through an API interface called ISteamUserAuth.
/// Check [login_isteam_user_auth]
///
/// https://github.com/Jessecar96/SteamBot/blob/e8e9e5fcd64ae35b201e2597068849c10a667b60/SteamTrade/SteamWeb.cs#L325
// We can really do that method yet, because connection to the SteamNetwork is not yet implemented
// by steam-client crate, and consequently we can't get the user webapi_nonce beforehand.
//
// Should accept closure to handle cases such as needing a captcha or sms.
// But the best way is to have it already setup to use TOTP codes.
#[allow(clippy::too_many_lines)]
pub(crate) async fn login_and_store_cookies<'a>(
    client: &MobileClient,
    user: &User,
    cached_data: Arc<RwLock<SteamCache>>,
) -> Result<(), LoginError> {
    // we request to generate sessionID cookies
    let response = client
        .request(MOBILE_REFERER.to_owned(), Method::GET, None, None::<u8>, None::<u8>)
        .await?;

    let session_id_cookie = response
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .map(|cookie| cookie.to_str().unwrap())
        .map(|c| {
            let index = c.find('=').unwrap();
            c[index + 1..index + 25].to_string()
        })
        .ok_or_else(|| {
            LoginError::GeneralFailure("Something went wrong while getting sessionid cookie. Please retry.".to_string())
        })?;

    let rsa_query_params = &[("account_name", &user.username)];
    let rsa_response = client
        .request_and_decode::<_, RSAResponseBase, _>(
            LOGIN_RSA_ENDPOINT.to_string(),
            Method::GET,
            None,
            None::<u8>,
            Some(&rsa_query_params),
        )
        .await?;

    // wait for steam to catch up
    Delay::new(Duration::from_millis(STEAM_DELAY_MS)).await;

    // handle encrypting user's password with RSA request from Steam login API
    let encrypted_pwd_b64 = encrypt_password(user, rsa_response.inner.publickey_mod, rsa_response.inner.publickey_exp);
    let encryption_timestamp = rsa_response.inner.timestamp;

    let payload =
        BeginAuthSessionViaCredentialsRequest::new(user.username.clone(), encrypted_pwd_b64, encryption_timestamp);
    let begin_auth_response = client
        .request_and_decode::<_, BeginAuthSessionViaCredentialsResponseBase, _>(
            LOGIN_BEGIN_AUTH_ENDPOINT.to_owned(),
            Method::POST,
            None,
            Some(payload),
            None::<&u8>,
        )
        .await?;

    if let Some(confirmations) = begin_auth_response.inner.allowed_confirmations.first() {
        if confirmations.confirmation_type != 3 {
            return Err(LoginError::Need2FA.into());
        }
    }

    Delay::new(Duration::from_millis(STEAM_DELAY_MS)).await;

    let offset = Time::offset().await?;
    let time = Time::now(Some(offset)).unwrap();

    // let steam_time_offset = (offset * 1000).to_string();
    let two_factor_code = user
        .linked_mafile
        .as_ref()
        .map(|f| Secret::from_b64(&f.shared_secret).unwrap())
        .map_or_else(String::new, |s| steam_totp::generate_auth_code(s, time));

    let payload = UpdateAuthSessionWithSteamGuardCodeRequest::from_begin_auth_response(
        begin_auth_response.clone(),
        two_factor_code,
    );
    client
        .request(
            LOGIN_UPDATE_STEAM_GUARD_ENDPOINT.to_owned(),
            Method::POST,
            None,
            Some(payload),
            None::<u8>,
        )
        .and_then(|resp| resp.text().err_into())
        .inspect_ok(|text| debug!("UpdateAuthSession response: {text}"))
        .await?;

    let payload = PollAuthSessionStatusRequest::from_begin_auth_response(begin_auth_response.inner);
    let poll_status_response = client
        .request_and_decode::<_, PollAuthSessionStatusResponseBase, _>(
            LOGIN_POLL_AUTH_STATUS_ENDPOINT.to_owned(),
            Method::POST,
            None,
            Some(payload),
            None::<u8>,
        )
        .await?;

    let inner = poll_status_response.inner;
    if inner.access_token.is_none() || inner.refresh_token.is_none() {
        return Err(LoginError::IncorrectCredentials);
    }

    // This next operation will fail if called too fast, we should wait a bit.
    Delay::new(Duration::from_millis(STEAM_DELAY_MS)).await;

    // let session_id_cookie = client
    //     .get_cookie_value(STEAM_COMMUNITY_BASE, SESSION_ID_COOKIE)
    //     .expect("SessionID cookie should be set.");

    let refresh_token = inner.refresh_token.expect("Safe to unwrap");
    let finalize_payload = FinalizeLoginRequest::new(refresh_token, session_id_cookie);

    let finalize_login_response = client
        .request_and_decode::<_, FinalizeLoginResponseBase, _>(
            LOGIN_FINALIZE_LOGIN_ENDPOINT.to_owned(),
            Method::POST,
            None,
            Some(finalize_payload),
            None::<u8>,
        )
        .await?;

    let domain_tokens = finalize_login_response.domain_tokens;
    let steam_id = finalize_login_response.steam_id;
    set_cookies_on_steam_domains(client, domain_tokens, steam_id).await;

    // {
    //     // Recover cookies to authorize store.steampowered and help.steampowered subdomains.
    //     let mut cookie_jar = client.cookie_store.write();
    //     for host in &[STEAM_COMMUNITY_HOST, STEAM_HELP_HOST, STEAM_STORE_HOST] {
    //         let timezone_offset = format!("{},0", chrono::Local::now().offset().fix().local_minus_utc());
    //         let fmt_token = format!("{steamid}%7C%7C{token}");
    //         let fmt_secure_token = format!("{steamid}%7C%7C{token_secure}");
    //         cookie_jar.add_original(
    //             Cookie::build("steamLoginSecure", fmt_secure_token)
    //                 .domain(*host)
    //                 .path("/")
    //                 .finish(),
    //         );
    //         cookie_jar.add_original(Cookie::build("steamLogin", fmt_token).domain(*host).path("/").finish());
    //         cookie_jar.add_original(
    //             Cookie::build("sessionid", session_id.clone())
    //                 .domain(*host)
    //                 .path("/")
    //                 .finish(),
    //         );
    //         cookie_jar.add_original(
    //             Cookie::build("timezoneOffset", timezone_offset)
    //                 .domain(*host)
    //                 .path("/")
    //                 .finish(),
    //         );
    //     }
    // }

    Ok(())
}

/// Calls multiple Steam Domains and set cookies for them.
pub async fn set_cookies_on_steam_domains(client: &MobileClient, domain_tokens: Vec<DomainToken>, steam_id: String) {
    let mut futures = domain_tokens
        .into_iter()
        .map(|c| {
            let url = c.url;
            let mut token_data = c.params;
            token_data.steam_id = Some(steam_id.clone());
            let payload = json!({"params": token_data});
            client.request(url, Method::POST, None, Some(payload), None::<u8>)
        })
        .collect::<FuturesUnordered<_>>();

    debug!("Setting tokens..");
    while let Some(fut) = futures.next().await {
        match fut {
            Err(err) => {
                error!("Error happened while setting tokens for all domains.\n{err}");
            }
            Ok(response) => {
                let mut cache = client.cookie_store.write();
                debug!("URL: {:?}", response.url());
                for cookie in response.cookies() {
                    debug!("cookie_name: {}, value: {}", cookie.name(), cookie.value());
                    let our_cookie = SteamCookie::from(cookie);
                    cache.add_original(our_cookie.deref().clone());
                }
            }
        }
    }
}