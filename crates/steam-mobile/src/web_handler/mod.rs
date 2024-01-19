use std::borrow::Cow;
use std::fmt::Write;
use std::sync::Arc;
use std::time::Duration;

use const_format::concatcp;
use cookie::Cookie;
use futures_timer::Delay;
use futures_util::TryFutureExt;
use reqwest::Method;
use steam_language_gen::generated::enums::EResult;
use steam_totp::Secret;
use steam_totp::Time;
use tracing::debug;
use tracing::error;
use tracing::warn;

use crate::client::MobileClient;
use crate::errors::ApiKeyError;
use crate::errors::InternalError;
use crate::errors::LoginError;
use crate::page_scraper::api_key_resolve_status;
use crate::types::ConfirmationResponseBase;
use crate::types::ParentalUnlockRequest;
use crate::types::ParentalUnlockResponse;
use crate::user::IsUser;
use crate::user::PresentMaFile;
use crate::user::SteamUser;
use crate::utils::dump_cookie_from_header;
use crate::utils::dump_cookies_by_domain_and_name;
use crate::web_handler::api_key::NewAPIKeyRequest;
use crate::web_handler::api_key::NewAPIKeyResponse;
use crate::web_handler::confirmation::Confirmation;
use crate::web_handler::confirmation::ConfirmationMethod;
use crate::web_handler::confirmation::ConfirmationTag;
use crate::web_handler::login::SESSION_ID_COOKIE;
use crate::Confirmations;
use crate::EConfirmationType;
use crate::Url;
use crate::STEAM_COMMUNITY_BASE;
use crate::STEAM_COMMUNITY_HOST;
use crate::STEAM_DELAY_MS;
use crate::STEAM_STORE_BASE;
use crate::STEAM_STORE_HOST;

pub mod api_key;
pub mod confirmation;
pub mod login;
pub mod steam_guard_linker;

const CONFIRMATIONS_GET_ENDPOINT: &str = concatcp!(STEAM_COMMUNITY_BASE, "/mobileconf/getlist");
const CONFIRMATIONS_SEND_ENDPOINT: &str = concatcp!(STEAM_COMMUNITY_BASE, "/mobileconf/multiajaxop");

// TODO: Refresh session for long-time running authenticators.
#[allow(clippy::unused_async)]
async fn session_refresh() {}

/// Parental unlock operation should be made otherwise any operation will fail and should be
/// performed immediately after login
pub async fn parental_unlock(client: &MobileClient, parental_code: &str) -> Result<(), LoginError> {
    // unlocks parental on steam community
    {
        parental_unlock_by_service(client, &parental_code, STEAM_COMMUNITY_BASE, STEAM_COMMUNITY_HOST).await?;
    }

    // unlocks parental on steam store
    {
        parental_unlock_by_service(client, &parental_code, STEAM_STORE_BASE, STEAM_STORE_HOST).await?;
    }
    Ok(())
}

/// Try to unlock account with parental controls (Family Sharing).
async fn parental_unlock_by_service(
    client: &MobileClient,
    parental_control_code: &str,
    url: &str,
    cookie_host: &str,
) -> Result<(), LoginError> {
    let unlock_url = format!("{url}/parental/ajaxunlock");
    let session_id = dump_cookies_by_domain_and_name(&client.cookie_store.read(), cookie_host, "sessionid").unwrap();

    let request = ParentalUnlockRequest {
        pin: parental_control_code,
        sessionid: &session_id,
    };
    let response = client
        .request(unlock_url, Method::POST, None, Some(&request), None::<u8>)
        .await?;

    let parental_cookie_name = "steamparental";
    if let Some(cookie) = dump_cookie_from_header(&response, parental_cookie_name) {
        let mut cookie_jar = client.cookie_store.write();
        cookie_jar.add_original(
            Cookie::build(parental_cookie_name, cookie.clone())
                .domain(STEAM_STORE_HOST)
                .path("/")
                .finish(),
        );
        cookie_jar.add_original(
            Cookie::build(parental_cookie_name, cookie)
                .domain(STEAM_COMMUNITY_HOST)
                .path("/")
                .finish(),
        );
    }

    // FIXME: Sometimes this fails, when consecutively logging in. Happened when tried 3 times.
    // We should try again
    let response = response.text().await.unwrap();

    let response = serde_json::from_str::<ParentalUnlockResponse>(&response)
        .map_err(|e| warn!("{}", e))
        .unwrap();

    if response.eresult != EResult::OK {
        let error = format!("EResult: {:?} {}", response.eresult, response.error_message);
        return Err(LoginError::ParentalUnlock(error));
    }

    Ok(())
}

/// Resolve caching of the user APIKey.
/// This is done after user logon for the first time in this session.
pub async fn cache_api_key(client: &MobileClient, user: Arc<dyn IsUser>, steamid: u64) -> Option<String> {
    let api_key_res = api_key_retrieve(client)
        .inspect_err(|e| warn!("API key could not be fetched.\nReason: {}", e))
        .await;

    match api_key_res {
        Ok(api) => Some(api),

        Err(ApiKeyError::NotRegistered) => {
            if let Some(user) = user.as_any().downcast_ref::<SteamUser<PresentMaFile>>() {
                warn!("API key not registered. Registering a new one.");
                api_key_register(client, user, steamid)
                    .await
                    .expect("TODO: panic message");
                // recursive cache?
                return None;
            }
            warn!("API key not registered.");
            None
        }
        Err(ApiKeyError::AccessDenied) => {
            warn!("Access to API key was denied. You need to spend at least 5USD on steam store to unlock it.");
            None
        }
        Err(e) => {
            warn!("Could not cache API Key. {}", e);
            None
        }
    }
}

/// Retrieve all confirmations for user, opting between retrieving details or not.
/// # Panics
/// This method will panic if the `User` doesn't have a linked `device_id`.
#[tracing::instrument(skip(client))]
pub async fn get_confirmations(
    client: &MobileClient,
    identity_secret: Secret,
    device_id: &str,
    steamid: u64,
) -> Result<Confirmations, InternalError> {
    let query_params =
        generate_confirmation_query_params(identity_secret, device_id, steamid, ConfirmationTag::Confirmation, None)
            .await;

    let confirmation_url = Url::parse(CONFIRMATIONS_GET_ENDPOINT).expect("Safe to unwrap");
    let response = client
        .request_and_decode::<_, ConfirmationResponseBase, _, _>(
            confirmation_url,
            Method::GET,
            None,
            None::<u8>,
            query_params,
        )
        .await?;

    debug!("Retrieved {} confirmations.", response.conf.len());
    Ok(Confirmations::from(response.conf))
}

/// Send confirmations to Steam Servers for accepting/denying.
///
/// # Panics
/// This method will panic if the `User` doesn't have a linked `device_id`.
#[tracing::instrument(skip(client, confirmations))]
pub async fn send_confirmations<I>(
    client: &MobileClient,
    identity_secret: Secret,
    device_id: &str,
    steamid: u64,
    method: ConfirmationMethod,
    confirmations: I,
) -> Result<(), InternalError>
where
    I: IntoIterator<Item = Confirmation> + Send,
{
    let url = Url::parse(CONFIRMATIONS_SEND_ENDPOINT).expect("Safe to unwrap");
    let query_params = generate_confirmation_query_params(
        identity_secret,
        device_id,
        steamid,
        ConfirmationTag::Confirmation,
        Some(method),
    )
    .await;

    let payload = confirmations
        .into_iter()
        .fold(String::new(), |mut buf, conf: Confirmation| {
            let _ = write!(buf, "&cid[]={}&ck[]={}", conf.id, conf.key);
            buf
        });
    let query_params = serde_qs::to_string(&query_params)
        .map(move |mut s| {
            s.push_str(&payload);
            s
        })
        .expect("Safe to unwrap");

    let resp = client
        .request(url, Method::POST, None, Some(query_params), None::<u8>)
        .and_then(|r| r.text().err_into())
        .await?;
    error!("Send resp: {resp}");

    Ok(())
}

async fn generate_confirmation_query_params(
    identity_secret: Secret,
    device_id: &str,
    steamid: u64,
    tag: ConfirmationTag,
    op: Option<ConfirmationMethod>,
) -> Vec<(&'static str, Cow<str>)> {
    let time = Time::with_offset().await.unwrap();
    let confirmation_hash = steam_totp::generate_confirmation_key(identity_secret, time, Some("conf")).unwrap();

    let mut params = vec![
        ("p", device_id.into()),
        ("a", steamid.to_string().into()),
        ("k", confirmation_hash.into()),
        ("t", time.to_string().into()),
        ("m", "react".into()),
        ("tag", tag.value().into()),
    ];

    if let Some(op) = op {
        params.push(("op", op.value().into()));
    }
    params
}

/// Retrieve this account API KEY.
/// If the API is not immediately available, but can be registered, the method will attempt to register it.
///
///
/// Will error only if an unknown or network error is raised.
async fn api_key_retrieve(client: &MobileClient) -> Result<String, ApiKeyError> {
    let api_key_url = format!("{}{}", STEAM_COMMUNITY_BASE, "/dev/apikey?l=english");
    let doc = client.get_html(api_key_url.clone(), None, None::<u8>).await?;
    api_key_resolve_status(doc)
}

/// Sends a request to enable an API Key for the account.
#[tracing::instrument(skip(client))]
async fn api_key_register(
    client: &MobileClient,
    user: &SteamUser<PresentMaFile>,
    steamid: u64,
) -> Result<(), ApiKeyError> {
    let api_register_url = format!("{}{}", STEAM_COMMUNITY_BASE, "/dev/requestkey");
    let session_id = client
        .get_cookie_value(STEAM_COMMUNITY_HOST, SESSION_ID_COOKIE)
        .unwrap();
    let register_request = NewAPIKeyRequest::new(session_id.clone());

    let response = client
        .request_and_decode::<_, NewAPIKeyResponse, _, _>(
            api_register_url,
            Method::POST,
            None,
            Some(register_request),
            None::<&str>,
        )
        .await?;

    // Doesn't require mobile confirmation?
    if response.requires_confirmation < 1 {}

    Delay::new(Duration::from_millis(STEAM_DELAY_MS)).await;

    let identity_secret = user.identity_secret();
    let device_id = user.device_id();
    let api_confirmation = get_confirmations(client, identity_secret.clone(), device_id, steamid)
        .await?
        .into_iter()
        .filter(|c| c.kind == EConfirmationType::APIKey);

    send_confirmations(
        client,
        identity_secret,
        device_id,
        steamid,
        ConfirmationMethod::Accept,
        api_confirmation,
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use reqwest::Url;

    #[test]
    fn test_path_handling() {
        let lost_auth = Url::parse("steammobile://lostauth/login").unwrap();
        assert_eq!("steammobile", lost_auth.scheme());
        assert_eq!("lostauth", lost_auth.host_str().unwrap());
        assert_eq!("/login", lost_auth.path());
    }
}
