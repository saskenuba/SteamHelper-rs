use std::cell::Ref;

use const_format::concatcp;
use futures::TryFutureExt;
use reqwest::Method;
use tokio::time::Duration;
use tracing::debug;

use crate::client::MobileClient;
use crate::errors::LinkerError;
use crate::utils::{dump_cookies_by_name, generate_canonical_device_id};
use crate::web_handler::authenticator::types::{
    AddAuthenticatorErrorResponseBase, AddAuthenticatorRequest, AddAuthenticatorResponseBase,
    FinalizeAddAuthenticatorBase, FinalizeAddAuthenticatorErrorBase, FinalizeAddAuthenticatorRequest,
    GenericSuccessResponse, HasPhoneResponse, PhoneAjaxRequest,
};
use crate::{CachedInfo, MobileAuthFile, STEAM_API_BASE, STEAM_COMMUNITY_BASE, STEAM_COMMUNITY_HOST};

mod types;

const PHONEAJAX_URL: &str = concatcp!(STEAM_COMMUNITY_BASE, "/steamguard/phoneajax");
pub(crate) const STEAM_ADD_PHONE_CATCHUP_SECS: u64 = 5;

type LinkerResult<T> = Result<T, LinkerError>;

/// By default, your MobileAuth file will always be printed to the terminal.
pub struct Authenticator {
    phone_number: String,
}

struct AuthenticatorOptions {
    save_path: String,
    print_output: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AddAuthenticatorStep {
    /// User is signin up for the first time.
    InitialStep,
    /// Authenticator is waiting user's email confirmation to allow Steam add phone number.
    EmailConfirmation,
    /// Authenticator succeded and retrieved `MobileAuthFile`.
    MobileAuth(MobileAuthFile),
}

/// Queries the `/steamguard/phoneajax` to check if the user has a phone number.
/// Returns true if user has already a phone registered.
pub(crate) async fn account_has_phone(client: &MobileClient) -> LinkerResult<bool> {
    let session_id = dump_cookies_by_name(&client.cookie_store.borrow(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();
    let payload = PhoneAjaxRequest::has_phone(&*session_id);

    let response: HasPhoneResponse = client
        .request_with_session_guard(PHONEAJAX_URL.to_owned(), Method::POST, None, Some(payload))
        .and_then(|x| x.json::<HasPhoneResponse>())
        .await?;

    Ok(response.user_has_phone)
}

pub(crate) async fn check_sms(client: &MobileClient, sms_code: &str) -> LinkerResult<bool> {
    let session_id = dump_cookies_by_name(&client.cookie_store.borrow(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();
    let payload = PhoneAjaxRequest::check_sms(&*session_id, sms_code);

    let response: GenericSuccessResponse = client
        .request_with_session_guard(PHONEAJAX_URL.to_owned(), Method::POST, None, Some(payload))
        .and_then(|x| x.json::<GenericSuccessResponse>())
        .await?;

    Ok(response.success)
}

/// Signals Steam that the user confirmed the phone add request email, and is ready for the next step.
/// Confirming the email allows `SteamAuthenticator` to register a new phone number to account.
pub(crate) async fn check_email_confirmation(client: &MobileClient) -> LinkerResult<bool> {
    let session_id = dump_cookies_by_name(&client.cookie_store.borrow(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();
    let payload = PhoneAjaxRequest::check_email_confirmation(&*session_id);

    let response: GenericSuccessResponse = client
        .request_with_session_guard(PHONEAJAX_URL.to_owned(), Method::POST, None, Some(payload))
        .and_then(|x| x.json::<GenericSuccessResponse>())
        .await?;

    Ok(response.success)
}

pub(crate) async fn add_phone_to_account(client: &MobileClient, phone_number: &str) -> LinkerResult<bool> {
    let session_id = dump_cookies_by_name(&client.cookie_store.borrow(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();

    let payload = PhoneAjaxRequest::add_phone(&*session_id, phone_number);

    let response: GenericSuccessResponse = client
        .request_with_session_guard(PHONEAJAX_URL.to_owned(), Method::POST, None, Some(payload))
        .and_then(|x| x.json::<GenericSuccessResponse>())
        .await?;

    Ok(response.success)
}

pub fn validate_phone_number(phone_number: &str) -> bool {
    phone_number.starts_with('+')
}

/// Last step to add a new authenticator.
pub(crate) async fn finalize_authenticator(
    client: &MobileClient,
    cached_data: Ref<'_, CachedInfo>,
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

        let response_text: String = client
            .request_with_session_guard(finalize_url.clone(), Method::POST, None, Some(&initial_payload))
            .and_then(|resp| resp.text())
            .await?;

        debug!("FinalizeAuthenticator raw response: {:#}", response_text);

        let response = match serde_json::from_str::<FinalizeAddAuthenticatorBase>(&*response_text) {
            Ok(resp) => resp.response,
            Err(err) => {
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
            tokio::time::delay_for(Duration::from_secs(1)).await;
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
    cached_data: Ref<'_, CachedInfo>,
) -> Result<MobileAuthFile, LinkerError> {
    let add_auth_url = format!("{}{}", STEAM_API_BASE, "/ITwoFactorService/AddAuthenticator/v0001");
    let oauth_token = cached_data.oauth_token().unwrap();
    let steamid = cached_data.steam_id().unwrap().to_string();
    let time = steam_totp::time::Time::with_offset().await?.to_string();

    let payload = AddAuthenticatorRequest::new(oauth_token, &*steamid, time.parse().unwrap());

    let response_text: String = client
        .request_with_session_guard(add_auth_url, Method::POST, None, Some(payload))
        .and_then(|resp| resp.text())
        .await?;

    debug!("Steam addauth raw response: {:?}", response_text);

    let mut mafile = match serde_json::from_str::<AddAuthenticatorResponseBase>(&*response_text) {
        Ok(resp) => resp.steam_guard_success_details.mobile_auth,
        Err(err) => {
            eprintln!("Error found deserializing add auth response: {:#?}", err);
            let error_resp = serde_json::from_str::<AddAuthenticatorErrorResponseBase>(&*response_text).unwrap();
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
    mafile.set_device_id(generate_canonical_device_id(&*steamid));
    Ok(mafile)
}

/// Remove authenticator from account.
pub(crate) async fn remove_authenticator(_client: &MobileClient, _cached_data: Ref<'_, CachedInfo>) {
    let _url = format!("{}{}", STEAM_API_BASE, "/ITwoFactorService/AddAuthenticator/v0001");
}
