use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use const_format::concatcp;
use downcast_rs::Downcast;
use futures_timer::Delay;
use futures_util::future::try_join_all;
use rand::thread_rng;
use reqwest::Client;
use reqwest::Method;
use rsa::BigUint;
use rsa::Pkcs1v15Encrypt;
use rsa::RsaPublicKey;
use steam_protobuf::protobufs::enums::ESessionPersistence;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_BeginAuthSessionViaCredentials_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_BeginAuthSessionViaCredentials_Response;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_PollAuthSessionStatus_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_PollAuthSessionStatus_Response;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_UpdateAuthSessionWithSteamGuardCode_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_UpdateAuthSessionWithSteamGuardCode_Response;
use steam_protobuf::protobufs::steammessages_auth_steamclient::EAuthSessionGuardType;
use steam_totp::Time;
use tracing::debug;

use crate::client::MobileClient;
use crate::errors::InternalError;
use crate::errors::LoginError;
use crate::types::DomainToken;
use crate::types::FinalizeLoginRequest;
use crate::types::FinalizeLoginResponseBase;
use crate::types::RSAResponseBase;
use crate::user::IsUser;
use crate::user::PresentMaFile;
use crate::user::SteamUser;
use crate::AuthResult;
use crate::SteamCache;
use crate::STEAM_API_BASE;
use crate::STEAM_COMMUNITY_HOST;
use crate::STEAM_DELAY_MS;
use crate::STEAM_LOGIN_BASE;

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

pub(crate) const SESSION_ID_COOKIE: &str = "sessionid";
pub(crate) const STEAM_LOGIN_SECURE_COOKIE: &str = "steamLoginSecure";

fn encrypt_password<MOD, EXP>(password: &str, modulus: MOD, exponent: EXP) -> String
where
    MOD: AsRef<[u8]>,
    EXP: AsRef<[u8]>,
{
    let password_bytes = password.as_bytes();
    let exponent = BigUint::parse_bytes(exponent.as_ref(), 16).unwrap();
    let modulus = BigUint::parse_bytes(modulus.as_ref(), 16).unwrap();

    let encrypted = RsaPublicKey::new(modulus, exponent)
        .expect("Failed to create public key.")
        .encrypt(&mut thread_rng(), Pkcs1v15Encrypt, password_bytes)
        .expect("Failed to encrypt.");

    base64::engine::general_purpose::STANDARD.encode(encrypted)
}

/// Logs in Steam through Steam `ISteamAuthUser` interface.
///
/// `Webapi_nonce` is received by connecting to the Steam Network.
///
/// Currently not possible without the implementation of the [steam-client] crate.
/// For website that currently works, check [login_and_store_cookies] method.
async fn login_isteam_user_auth<U>(_client: &Client, _user: SteamUser<U>, _webapi_nonce: &[u8]) -> AuthResult<()> {
    unimplemented!();
}

/// Logs in through the Steam website, caching the user's SteamID,
/// and storing session cookies for steamcommunity and steampowered domains.
///
/// Additionally, there is a method for logging in through an API interface called ISteamUserAuth.
/// For more details, refer to [`login_isteam_user_auth`].
///
/// For the implementation details, you can check the original C# source code [here](https://github.com/Jessecar96/SteamBot/blob/e8e9e5fcd64ae35b201e2597068849c10a667b60/SteamTrade/SteamWeb.cs#L325).
///
/// [login_isteam_user_auth]: #method.login_isteam_user_auth
#[allow(clippy::too_many_lines)]
pub async fn login_and_store_cookies(client: &MobileClient, user: Arc<dyn IsUser>) -> Result<SteamCache, LoginError> {
    let rsa_query_params = &[("account_name", user.username())];
    let rsa_response = client
        .request_and_decode::<_, RSAResponseBase, _, _>(
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
    let encrypted_pwd_b64 = encrypt_password(
        user.password(),
        rsa_response.inner.publickey_mod,
        rsa_response.inner.publickey_exp,
    );
    let encryption_timestamp = rsa_response.inner.timestamp;

    let mut payload = CAuthentication_BeginAuthSessionViaCredentials_Request::new();
    payload.set_account_name(user.username().to_owned());
    payload.set_encrypted_password(encrypted_pwd_b64);
    payload.set_encryption_timestamp(encryption_timestamp.parse().unwrap());
    payload.set_persistence(ESessionPersistence::k_ESessionPersistence_Persistent);
    let begin_auth_response = client
        .request_proto::<_, CAuthentication_BeginAuthSessionViaCredentials_Response>(
            LOGIN_BEGIN_AUTH_ENDPOINT.to_owned(),
            Method::POST,
            payload,
            None,
        )
        .await?;

    Delay::new(Duration::from_millis(STEAM_DELAY_MS)).await;

    let client_id = begin_auth_response.client_id();
    let steam_id = begin_auth_response.steamid();
    let request_id = begin_auth_response.request_id().to_vec();

    // TODO: implement a way for email codes?
    let mut payload = CAuthentication_UpdateAuthSessionWithSteamGuardCode_Request::new();
    payload.set_client_id(client_id);
    payload.set_steamid(steam_id);

    if let Some(ma_user) = user.as_any().downcast_ref::<SteamUser<PresentMaFile>>() {
        let offset = Time::offset().await?;
        let time = Time::now(Some(offset)).unwrap();
        let two_factor_code = steam_totp::generate_auth_code(ma_user.shared_secret(), time);

        payload.set_code_type(EAuthSessionGuardType::k_EAuthSessionGuardType_DeviceCode);
        payload.set_code(two_factor_code);
    } else {
        payload.set_code_type(EAuthSessionGuardType::k_EAuthSessionGuardType_None);
    }

    let _updateauth_response = client
        .request_proto::<_, CAuthentication_UpdateAuthSessionWithSteamGuardCode_Response>(
            LOGIN_UPDATE_STEAM_GUARD_ENDPOINT.to_owned(),
            Method::POST,
            payload,
            None,
        )
        .await?;

    let mut payload = CAuthentication_PollAuthSessionStatus_Request::new();
    payload.set_client_id(client_id);
    payload.set_request_id(request_id.into());

    let poll_session_response = client
        .request_proto::<_, CAuthentication_PollAuthSessionStatus_Response>(
            LOGIN_POLL_AUTH_STATUS_ENDPOINT.to_owned(),
            Method::POST,
            payload,
            None,
        )
        .await?;

    if poll_session_response.access_token.is_none() || poll_session_response.refresh_token.is_none() {
        return Err(LoginError::IncorrectCredentials);
    }

    // This next operation will fail if called too fast, we should wait a bit.
    Delay::new(Duration::from_millis(STEAM_DELAY_MS)).await;

    let refresh_token = poll_session_response.refresh_token.expect("Safe to unwrap");
    let access_token = poll_session_response.access_token.expect("Safe to unwrap");

    // We should have the session_id cookie by now
    let session_id = client
        .get_cookie_value(STEAM_COMMUNITY_HOST, SESSION_ID_COOKIE)
        .unwrap();
    let finalize_payload = FinalizeLoginRequest::new(refresh_token.clone(), session_id);

    let finalize_login_response = client
        .request_and_decode::<_, FinalizeLoginResponseBase, _, _>(
            LOGIN_FINALIZE_LOGIN_ENDPOINT.to_owned(),
            Method::POST,
            None,
            Some(finalize_payload),
            None::<u8>,
        )
        .await?;

    let domain_tokens = finalize_login_response.domain_tokens;
    let steam_id = finalize_login_response.steam_id;
    set_cookies_on_steam_domains(client, domain_tokens, steam_id.clone()).await?;
    Ok(SteamCache::with_login_data(&steam_id, access_token, refresh_token).expect("Safe to unwrap"))
}

/// Calls multiple Steam Domains and set cookies for them.
pub async fn set_cookies_on_steam_domains(
    client: &MobileClient,
    domain_tokens: Vec<DomainToken>,
    steam_id: String,
) -> Result<(), InternalError> {
    let futures = domain_tokens
        .into_iter()
        .map(|c| {
            let url = c.url;
            let mut token_data = c.params;
            token_data.steam_id = Some(steam_id.clone());
            client.request(url, Method::POST, None, Some(token_data), None::<u8>)
        })
        .collect::<Vec<_>>();

    debug!("Setting tokens for all Steam domains..");
    try_join_all(futures).await.map(|_| ())
}
