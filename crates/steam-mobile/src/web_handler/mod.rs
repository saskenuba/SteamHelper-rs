use const_format::concatcp;
use cookie::Cookie;
use futures::FutureExt;
use futures_util::TryFutureExt;
use reqwest::Method;
use scraper::Html;
use steam_language_gen::generated::enums::EResult;
use steam_totp::Time;
use tracing::debug;
use tracing::trace;
use tracing::warn;

use crate::client::MobileClient;
use crate::errors::ApiKeyError;
use crate::errors::InternalError;
use crate::errors::LoginError;
use crate::page_scraper::api_key_resolve_status;
use crate::page_scraper::confirmation_details_single;
use crate::page_scraper::confirmation_retrieve;
use crate::types::ApiKeyRegisterRequest;
use crate::types::BooleanResponse;
use crate::types::ConfirmationDetailsResponse;
use crate::types::ConfirmationMultiAcceptRequest;
use crate::types::ParentalUnlockRequest;
use crate::types::ParentalUnlockResponse;
use crate::utils::dump_cookie_from_header;
use crate::utils::dump_cookies_by_domain_and_name;
use crate::web_handler::confirmation::Confirmation;
use crate::web_handler::confirmation::ConfirmationMethod;
use crate::User;
use crate::STEAM_API_BASE;
use crate::STEAM_COMMUNITY_BASE;
use crate::STEAM_COMMUNITY_HOST;
use crate::STEAM_STORE_BASE;
use crate::STEAM_STORE_HOST;

pub mod confirmation;
pub mod login;
pub mod steam_guard_linker;

/// used to refresh session
const MOBILE_AUTH_GETWGTOKEN: &str = concatcp!(STEAM_API_BASE, "/IMobileAuthService/GetWGToken/v0001");

// TODO: Refresh session for long-time running authenticators.
#[allow(clippy::unused_async)]
async fn session_refresh() {}

/// Parental unlock operation should be made otherwise any operation will fail and should be
/// performed immediately after login
pub async fn parental_unlock(client: &MobileClient, user: &User) -> Result<(), LoginError> {
    let parental_code = user.parental_code.clone().unwrap();

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
pub async fn cache_api_key(client: &MobileClient) -> Result<Option<String>, ApiKeyError> {
    api_key_retrieve(client)
        .inspect_err(|_e| warn!("API key could not be fetched."))
        .await
}

/// Send confirmations to Steam Servers for accepting/denying.
///
/// # Panics
/// This method will panic if the `User` doesn't have a linked `device_id`.
pub async fn confirmations_send<I>(
    client: &MobileClient,
    user: &User,
    steamid: u64,
    method: ConfirmationMethod,
    confirmations: I,
) -> Result<(), InternalError>
where
    I: IntoIterator<Item = Confirmation>,
{
    let url = format!("{STEAM_COMMUNITY_BASE}/mobileconf/multiajaxop");
    let operation = method.value();

    let mut id_vec = vec![];
    let mut key_vec = vec![];
    for confirmation in confirmations {
        id_vec.push(("cid[]", confirmation.id));
        key_vec.push(("ck[]", confirmation.key));
    }

    let (time, confirmation_hash, device_id) = generate_confirmation_query_params(user).await;
    let request = ConfirmationMultiAcceptRequest {
        steamid: &steamid.to_string(),
        confirmation_hash,
        operation,
        device_id,
        time: &time.to_string(),
        confirmation_id: id_vec,
        confirmation_key: key_vec,
        ..Default::default()
    };

    client
        .request_with_session_guard(url, Method::POST, None, Some(request), None::<&str>)
        .await?
        .json::<BooleanResponse>()
        .await?;

    // FIXME: Error Catching
    // if response.success {
    //     Ok(())
    // }

    Ok(())
}

/// Retrieve all confirmations for user, opting between retrieving details or not.
/// # Panics
/// This method will panic if the `User` doesn't have a linked `device_id`.
pub(crate) async fn confirmations_retrieve_all(
    client: &MobileClient,
    user: &User,
    steamid: u64,
    require_details: bool,
) -> Result<Option<Vec<Confirmation>>, InternalError> {
    let (time, confirmation_hash, device_id) = generate_confirmation_query_params(user).await;

    let confirmation_all_url = format!(
        "{STEAM_COMMUNITY_BASE}/mobileconf/conf?a={}&k={}&l=english&m=android&p={}&t={}&tag=conf",
        steamid, confirmation_hash, device_id, time
    );
    trace!("Confirmation url: {}", confirmation_all_url);

    let html = client.get_html(confirmation_all_url).await?;
    let user_confirmations = confirmation_retrieve(html);

    // There is no need for now for additional details of the confirmation..
    if !require_details || user_confirmations.is_none() {
        return Ok(user_confirmations);
    }

    // FIXME: Is there a need to fetch additional details?
    // We are not using this for anything yet

    let mut user_confirmations = user_confirmations.unwrap();
    let conf_details_fut = user_confirmations
        .iter()
        .map(|confirmation| {
            let details_url = format!(
                "{}/mobileconf/details/{}?a={}&k={}&l=english&m=android&p={}&t={}&tag=conf",
                STEAM_COMMUNITY_BASE, confirmation.id, steamid, confirmation_hash, device_id, time
            );
            client.request(details_url, Method::GET, None, None::<u8>, None::<u8>)
        })
        .collect::<Vec<_>>();

    let joined_fut: Vec<Result<reqwest::Response, _>> = futures::future::join_all(conf_details_fut).await;
    let mut details_vec = Vec::new();
    for response_res in joined_fut {
        let response_content = match response_res {
            Err(err) => {
                warn!("Failed to fetch details page for confirmation: {}\nSkipping..", err);
                continue;
            }
            Ok(response) => {
                let deserialized = response.json::<ConfirmationDetailsResponse>().await;
                if let Err(err) = deserialized {
                    warn!(
                        "Failed to deserialize confirmation details response: {}\nSkipping..",
                        err
                    );
                    continue;
                }
                deserialized.unwrap()
            }
        };

        let html = Html::parse_document(&response_content.html);
        details_vec.push(confirmation_details_single(&html));
    }

    for (confirmation, detail) in user_confirmations.iter_mut().zip(details_vec.into_iter()) {
        confirmation.details = Some(detail);
    }

    Ok(Some(user_confirmations))
}

async fn generate_confirmation_query_params(user: &User) -> (Time, String, &str) {
    let time = Time::with_offset().await.unwrap();
    let identity_secret = user
        .identity_secret()
        .expect("You need to have a linked ma file to recover confirmations");
    let confirmation_hash = steam_totp::generate_confirmation_key(identity_secret, time, Some("conf")).unwrap();
    let device_id = user.device_id().expect("You need a linked device id");
    (time, confirmation_hash, device_id)
}

/// Retrieve this account API KEY.
/// If the API is not immediately available, but can be registered, the method will attempt to register it.
///
///
/// Will error only if an unknown or network error is raised.
async fn api_key_retrieve(client: &MobileClient) -> Result<Option<String>, ApiKeyError> {
    let api_key_url = format!("{}{}", STEAM_COMMUNITY_BASE, "/dev/apikey?l=english");
    let doc = client.get_html(api_key_url.clone()).await?;
    Ok(match api_key_resolve_status(doc) {
        Ok(api) => Some(api),

        Err(ApiKeyError::NotRegistered) => {
            warn!("API key not registered. Registering a new one.");
            api_key_register(client)
                .then(|_| client.get_html(api_key_url))
                .await
                .map(api_key_resolve_status)?
                .ok()
        }
        Err(ApiKeyError::AccessDenied) => {
            warn!("Access to API key was denied. Maybe you don't have a valid email address?");
            None
        }
        Err(e) => {
            warn!("Could not cache API Key. {}", e);
            None
        }
    })
}

/// Request access to an API Key
/// The account should be validated before.
async fn api_key_register(client: &MobileClient) -> Result<(), ApiKeyError> {
    let api_register_url = format!("{}{}", STEAM_COMMUNITY_BASE, "/dev/registerkey");
    let register_request = ApiKeyRegisterRequest::default();

    let response = client
        .request_with_session_guard(
            api_register_url,
            Method::POST,
            None,
            Some(register_request),
            None::<&str>,
        )
        .await?;
    debug!("{:?}", response);

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
