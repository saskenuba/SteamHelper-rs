use std::time::Duration;

use const_format::concatcp;
use futures::TryFutureExt;
use futures_timer::Delay;
use parking_lot::lock_api::RwLockReadGuard;
use parking_lot::RawRwLock;
use reqwest::Method;
use tracing::debug;

use crate::client::MobileClient;
use crate::errors::{AuthError, LinkerError};
use crate::utils::{dump_cookies_by_domain_and_name, generate_canonical_device_id};
use crate::web_handler::steam_guard_linker::types::{
    AddAuthenticatorErrorResponseBase, AddAuthenticatorRequest, AddAuthenticatorResponseBase,
    FinalizeAddAuthenticatorBase, FinalizeAddAuthenticatorErrorBase, FinalizeAddAuthenticatorRequest,
    GenericSuccessResponse, HasPhoneResponse, PhoneAjaxRequest, RemoveAuthenticatorRequest,
    RemoveAuthenticatorResponseBase,
};
use crate::{SteamCache, MobileAuthFile, STEAM_API_BASE, STEAM_COMMUNITY_BASE, STEAM_COMMUNITY_HOST};

mod types;

const PHONEAJAX_URL: &str = concatcp!(STEAM_COMMUNITY_BASE, "/steamguard/phoneajax");
pub const STEAM_ADD_PHONE_CATCHUP_SECS: u64 = 5;

type LinkerResult<T> = Result<T, LinkerError>;

/// By default, your `MobileAuth` file will always be printed to the terminal.
pub struct Authenticator {
    phone_number: String,
}

struct AuthenticatorOptions {
    save_path: String,
    print_output: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddAuthenticatorStep {
    /// The user is signing up for the first time.
    InitialStep,
    /// The authenticator is awaiting the user's email confirmation to enable the addition of a phone number to Steam.
    EmailConfirmation,
    /// Authenticator succeeded and retrieved `MobileAuthFile`.
    MobileAuth(MobileAuthFile),
}

/// Queries the `/steamguard/phoneajax` to check if the user has a phone number.
/// Returns true if user has already a phone registered.
pub async fn account_has_phone(client: &MobileClient) -> LinkerResult<bool> {
    let session_id =
        dump_cookies_by_domain_and_name(&client.cookie_store.read(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();
    let payload = PhoneAjaxRequest::has_phone(&*session_id);

    let response: HasPhoneResponse = client
        .request_with_session_guard(PHONEAJAX_URL.to_owned(), Method::POST, None, Some(payload))
        .and_then(|x| x.json::<HasPhoneResponse>().err_into())
        .await?;

    Ok(response.user_has_phone)
}

pub async fn check_sms(client: &MobileClient, sms_code: &str) -> LinkerResult<bool> {
    let session_id =
        dump_cookies_by_domain_and_name(&client.cookie_store.read(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();
    let payload = PhoneAjaxRequest::check_sms(&session_id, sms_code);

    let response = client
        .request_with_session_guard_and_decode::<_, GenericSuccessResponse>(
            PHONEAJAX_URL.to_owned(),
            Method::POST,
            None,
            Some(payload),
        )
        .await?;

    Ok(response.success)
}

/// Signals Steam that the user confirmed the phone add request email, and is ready for the next step.
/// Confirming the email allows `SteamAuthenticator` to register a new phone number to account.
pub async fn check_email_confirmation(client: &MobileClient) -> LinkerResult<bool> {
    let session_id =
        dump_cookies_by_domain_and_name(&client.cookie_store.read(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();
    let payload = PhoneAjaxRequest::check_email_confirmation(&*session_id);

    let response: GenericSuccessResponse = client
        .request_with_session_guard_and_decode::<_, GenericSuccessResponse>(
            PHONEAJAX_URL.to_owned(),
            Method::POST,
            None,
            Some(payload),
        )
        .await?;

    Ok(response.success)
}

pub async fn add_phone_to_account(client: &MobileClient, phone_number: &str) -> LinkerResult<bool> {
    let session_id =
        dump_cookies_by_domain_and_name(&client.cookie_store.read(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();

    let payload = PhoneAjaxRequest::add_phone(&*session_id, phone_number);

    let response: GenericSuccessResponse = client
        .request_with_session_guard_and_decode::<_, GenericSuccessResponse>(
            PHONEAJAX_URL.to_owned(),
            Method::POST,
            None,
            Some(payload),
        )
        .await?;

    Ok(response.success)
}

pub fn validate_phone_number(phone_number: &str) -> bool {
    phone_number.starts_with('+')
}

/// Last step to add a new authenticator.
pub(crate) async fn finalize(
    client: &MobileClient,
    cached_data: RwLockReadGuard<'_, RawRwLock, SteamCache>,
    mafile: &MobileAuthFile,
    sms_code: &str,
) -> LinkerResult<()> {
    let steamid = cached_data.steam_id().expect("This should be cached. Bug?").to_string();
    let oauth_token = cached_data.oauth_token().expect("This should be cached. Bug?");

    let finalize_url = format!(
        "{}{}",
        STEAM_API_BASE, "/ITwoFactorService/FinalizeAddAuthenticator/v0001"
    );

    let mut initial_payload = FinalizeAddAuthenticatorRequest {
        steamid: &*steamid,
        oauth_token,
        sms_activation_code: sms_code,
        ..Default::default()
    };

    let account_secret = steam_totp::Secret::from_b64(&mafile.shared_secret).unwrap();

    let mut tries: usize = 0;
    while tries <= 30 {
        let (code, mut time) = steam_totp::generate_auth_code_with_time_async(account_secret.clone()).await?;
        time.0 += 1;
        initial_payload.swap_codes(code, time.0);

        let response_text = client
            .request_with_session_guard(finalize_url.clone(), Method::POST, None, Some(&initial_payload))
            .and_then(|resp| resp.text().err_into())
            .await?;

        debug!("FinalizeAuthenticator raw response: {:#}", response_text);

        let response = match serde_json::from_str::<FinalizeAddAuthenticatorBase>(&*response_text) {
            Ok(resp) => resp.response,
            Err(_err) => {
                let error_resp = serde_json::from_str::<FinalizeAddAuthenticatorErrorBase>(&*response_text).unwrap();
                return match error_resp.response.status {
                    89 => Err(LinkerError::BadSMSCode),
                    88 => {
                        if tries == 30 {
                            return Err(LinkerError::UnableToGenerateCorrectCodes);
                        }
                        continue;
                    }
                    _ => Err(LinkerError::GeneralFailure("Something went wrong".to_string())),
                };
            }
        };

        // Steam want more codes, delay a bit and send all again.
        if response.want_more {
            Delay::new(Duration::from_secs(1)).await;
            tries += 1;
            continue;
        }

        return Ok(());
    }

    Err(LinkerError::GeneralFailure(
        "Maximum tries achieved. Something went wrong.".to_string(),
    ))
}

/// Add a new authenticator to this steam account.
///
///
/// User will receive a SMS message, with the code required to finalize registering the new authenticator.
/// Returns the VERY important `SteamGuardAccount`, that must be saved before the next step(finalize auth) is completed,
/// otherwise the user is on risk of losing the account, since the `revocation_code` will also be lost.
pub(crate) async fn add_authenticator_to_account(
    client: &MobileClient,
    cached_data: RwLockReadGuard<'_, RawRwLock, SteamCache>,
) -> Result<MobileAuthFile, LinkerError> {
    let add_auth_url = format!("{}{}", STEAM_API_BASE, "/ITwoFactorService/AddAuthenticator/v0001");
    let oauth_token = cached_data.oauth_token().unwrap();
    let steamid = cached_data.steam_id().unwrap().to_string();
    let time = steam_totp::time::Time::with_offset().await?.to_string();

    let payload = AddAuthenticatorRequest::new(oauth_token, &steamid, time.parse().unwrap());

    let response_text = client
        .request_with_session_guard(add_auth_url, Method::POST, None, Some(payload))
        .and_then(|resp| resp.text().err_into())
        .await?;

    debug!("Steam addauth raw response: {:?}", response_text);

    let mut mafile = match serde_json::from_str::<AddAuthenticatorResponseBase>(&response_text) {
        Ok(resp) => resp.steam_guard_success_details.mobile_auth,
        Err(err) => {
            eprintln!("Error found deserializing add auth response: {:#?}", err);
            let error_resp = serde_json::from_str::<AddAuthenticatorErrorResponseBase>(&response_text).unwrap();
            return match error_resp.response.status {
                29 => Err(LinkerError::AuthenticatorPresent),
                2 => Err(LinkerError::GeneralFailure(
                    "After too many failed attempts, this may be a lock on the phone number or account. Going to test \
                     this tomorrow."
                        .to_string(),
                )),
                _ => Err(LinkerError::GeneralFailure("Something went wrong".to_string())),
            };
        }
    };
    mafile.set_device_id(generate_canonical_device_id(&steamid));
    Ok(mafile)
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum RemoveAuthenticatorScheme {
    ReturnToEmailCodes,
    RemoveSteamGuard,
}

impl RemoveAuthenticatorScheme {
    fn as_str(self) -> &'static str {
        match self {
            RemoveAuthenticatorScheme::ReturnToEmailCodes => "1",
            RemoveAuthenticatorScheme::RemoveSteamGuard => "2",
        }
    }
}

/// Remove authenticator from account.
pub(crate) async fn remove_authenticator(
    client: &MobileClient,
    cached_data: RwLockReadGuard<'_, RawRwLock, SteamCache>,
    revocation_token: &str,
    remove_authenticator_scheme: RemoveAuthenticatorScheme,
) -> Result<(), AuthError> {
    let _url = format!(
        "{}{}",
        STEAM_API_BASE, "/ITwoFactorService/RemoveAuthenticator/v1?access_token="
    );

    let steamid = cached_data
        .steam_id()
        .map(|s| s.to_string())
        .expect("Should have been set.");
    let oauth_token = cached_data.oauth_token().expect("Should have been set.");
    let payload = RemoveAuthenticatorRequest::new(oauth_token, steamid, revocation_token, remove_authenticator_scheme);

    // FIXME: add error handling and error variants
    client
        .request_with_session_guard_and_decode::<_, RemoveAuthenticatorResponseBase>(
            _url,
            Method::POST,
            None,
            Some(payload),
        )
        .await
        .map(|_| ())
        .map_err(Into::into)
}
