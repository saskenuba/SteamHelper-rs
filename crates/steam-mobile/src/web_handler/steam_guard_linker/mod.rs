use std::time::Duration;

use const_format::concatcp;
use futures::TryFutureExt;
use futures_timer::Delay;
use reqwest::Method;
use tracing::debug;

use crate::client::MobileClient;
use crate::errors::InternalError;
use crate::errors::LinkerError;
use crate::utils::dump_cookies_by_domain_and_name;
use crate::utils::generate_canonical_device_id;
use crate::web_handler::steam_guard_linker::types::AddAuthenticatorErrorResponseBase;
use crate::web_handler::steam_guard_linker::types::AddAuthenticatorRequest;
use crate::web_handler::steam_guard_linker::types::AddAuthenticatorResponseBase;
use crate::web_handler::steam_guard_linker::types::FinalizeAddAuthenticatorBase;
use crate::web_handler::steam_guard_linker::types::FinalizeAddAuthenticatorErrorBase;
use crate::web_handler::steam_guard_linker::types::FinalizeAddAuthenticatorRequest;
use crate::web_handler::steam_guard_linker::types::GenericSuccessResponse;
use crate::web_handler::steam_guard_linker::types::HasPhoneResponse;
use crate::web_handler::steam_guard_linker::types::PhoneAjaxRequest;
use crate::web_handler::steam_guard_linker::types::QueryStatusResponseBase;
use crate::web_handler::steam_guard_linker::types::RemoveAuthenticatorRequest;
use crate::web_handler::steam_guard_linker::types::RemoveAuthenticatorResponseBase;
use crate::MobileAuthFile;
use crate::SteamCache;
use crate::STEAM_API_BASE;
use crate::STEAM_COMMUNITY_BASE;
use crate::STEAM_COMMUNITY_HOST;

mod types;

pub use types::QueryStatusResponse;

const PHONEAJAX_URL: &str = concatcp!(STEAM_COMMUNITY_BASE, "/steamguard/phoneajax");
pub const STEAM_ADD_PHONE_CATCHUP_SECS: u64 = 5;
/// How many codes we send to `FinalizeAddAuthenticator` before giving up.
const FINALIZE_MAX_TRIES: usize = 30;
/// Status returned when the authenticator code we sent is not the one Steam expected.
const STATUS_TWO_FACTOR_CODE_MISMATCH: i64 = 88;
/// Status returned when the SMS activation code is wrong.
const STATUS_TWO_FACTOR_ACTIVATION_CODE_MISMATCH: i64 = 89;

type LinkerResult<T> = Result<T, LinkerError>;

/// By default, your `MobileAuth` file will always be printed to the terminal.
pub struct Authenticator {
    phone_number: String,
}

struct AuthenticatorOptions {
    save_path: String,
    print_output: bool,
}

/// Steps to add an authenticator to a Steam Account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddAuthenticatorStep {
    /// The user is signing up for the first time.
    InitialStep,
    /// The authenticator is awaiting the user's email confirmation to enable the addition of a phone number to Steam.
    EmailConfirmation,
    /// Authenticator succeeded and retrieved `MobileAuthFile`.
    MobileAuth(MobileAuthFile),
}

/// An authenticator that was added to the account, but not finalized yet.
pub(crate) struct PendingAuthenticator {
    pub mafile: MobileAuthFile,
    pub code_delivery: ActivationCodeDelivery,
}

/// Where Steam sent the activation code for a new authenticator.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActivationCodeDelivery {
    /// By SMS, to the account's phone. The hint is the end of the phone number, if Steam gave it.
    Sms {
        /// The end of the phone number.
        phone_hint: Option<String>,
    },
    /// By email, to the account's address.
    Email,
    /// Steam answered with a delivery kind we don't know.
    Unknown(i64),
}

impl ActivationCodeDelivery {
    fn from_confirm_type(confirm_type: Option<i64>, phone_hint: Option<String>) -> Self {
        match confirm_type {
            Some(1) => Self::Sms {
                phone_hint: phone_hint.filter(|hint| !hint.is_empty()),
            },
            Some(3) => Self::Email,
            other => Self::Unknown(other.unwrap_or_default()),
        }
    }
}

const QUERY_STATUS_ENDPOINT: &str = concatcp!(STEAM_API_BASE, "/ITwoFactorService/QueryStatus/v1/");

/// Queries Steam API to check SteamGuard Status.
pub(crate) async fn twofactor_status(client: &MobileClient, cache: &SteamCache) -> LinkerResult<QueryStatusResponse> {
    let steamid = &[("steamid", cache.steamid.to_steam64())];
    let query = cache.query_tokens();

    let response = client
        .request_with_session_guard_and_decode::<_, _, QueryStatusResponseBase>(
            QUERY_STATUS_ENDPOINT.to_owned(),
            Method::POST,
            None,
            Some(steamid),
            Some(query),
        )
        .await?
        .inner;

    Ok(response)
}

/// Queries the `/steamguard/phoneajax` to check if the user has a phone number.
/// Returns true if user has already a phone registered.
pub async fn account_has_phone(client: &MobileClient) -> LinkerResult<bool> {
    let session_id =
        dump_cookies_by_domain_and_name(&client.cookie_store.read(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();
    let payload = PhoneAjaxRequest::has_phone(&session_id);

    let response: HasPhoneResponse = client
        .request_with_session_guard(
            PHONEAJAX_URL.to_owned(),
            Method::POST,
            None,
            Some(payload),
            None::<&str>,
        )
        .and_then(|x| x.json::<HasPhoneResponse>().err_into())
        .await?;

    Ok(response.user_has_phone)
}

pub async fn check_sms(client: &MobileClient, sms_code: &str) -> LinkerResult<bool> {
    let session_id =
        dump_cookies_by_domain_and_name(&client.cookie_store.read(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();
    let payload = PhoneAjaxRequest::check_sms(&session_id, sms_code);

    let response = client
        .request_with_session_guard_and_decode::<_, _, GenericSuccessResponse>(
            PHONEAJAX_URL.to_owned(),
            Method::POST,
            None,
            Some(payload),
            None::<&str>,
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
        .request_with_session_guard_and_decode::<_, _, GenericSuccessResponse>(
            PHONEAJAX_URL.to_owned(),
            Method::POST,
            None,
            Some(payload),
            None::<&str>,
        )
        .await?;

    Ok(response.success)
}

pub async fn add_phone_to_account(client: &MobileClient, phone_number: &str) -> LinkerResult<bool> {
    let session_id =
        dump_cookies_by_domain_and_name(&client.cookie_store.read(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();

    let payload = PhoneAjaxRequest::add_phone(&*session_id, phone_number);

    let response = client
        .request_with_session_guard_and_decode::<_, _, GenericSuccessResponse>(
            PHONEAJAX_URL.to_owned(),
            Method::POST,
            None,
            Some(payload),
            None::<&str>,
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
    cached_data: &SteamCache,
    mafile: &MobileAuthFile,
    sms_code: &str,
) -> LinkerResult<()> {
    let steamid = cached_data.steam_id().to_string();
    let oauth_token = cached_data.oauth_token();

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

    let account_secret = steam_totp::Secret::from_b64(&mafile.shared_secret)?;

    for _ in 0..FINALIZE_MAX_TRIES {
        let (code, mut time) = steam_totp::generate_auth_code_with_time_async(account_secret.clone()).await?;
        time.0 += 1;
        initial_payload.swap_codes(code, time.0);

        let response_text = client
            .request_with_session_guard(
                finalize_url.clone(),
                Method::POST,
                None,
                Some(&initial_payload),
                None::<&str>,
            )
            .and_then(|resp| resp.text().err_into())
            .await?;

        debug!("FinalizeAuthenticator raw response: {:#}", response_text);

        let (success, status, want_more) = match serde_json::from_str::<FinalizeAddAuthenticatorBase>(&response_text) {
            Ok(resp) => (resp.response.success, resp.response.status, resp.response.want_more),
            Err(_) => {
                let error_resp = serde_json::from_str::<FinalizeAddAuthenticatorErrorBase>(&response_text)
                    .map_err(InternalError::from)?;
                (false, error_resp.response.status, false)
            }
        };

        if success {
            if !want_more {
                return Ok(());
            }
            // Steam wants more codes to make sure our generator works, delay a bit and send another.
            Delay::new(Duration::from_secs(1)).await;
            continue;
        }

        match status {
            STATUS_TWO_FACTOR_ACTIVATION_CODE_MISMATCH => return Err(LinkerError::BadSMSCode),
            STATUS_TWO_FACTOR_CODE_MISMATCH => Delay::new(Duration::from_secs(1)).await,
            status => {
                return Err(LinkerError::GeneralFailure(format!(
                    "FinalizeAddAuthenticator failed with status {status}."
                )))
            }
        }
    }

    Err(LinkerError::UnableToGenerateCorrectCodes)
}

/// Add a new authenticator to this steam account.
///
/// Steam sends the code required to finalize it by SMS if the account has a phone, or by email otherwise.
/// Returns the VERY important maFile, that must be saved before the next step(finalize auth) is completed,
/// otherwise the user is on risk of losing the account, since the `revocation_code` will also be lost.
pub(crate) async fn add_authenticator_to_account(
    client: &MobileClient,
    cached_data: &SteamCache,
) -> Result<PendingAuthenticator, LinkerError> {
    let add_auth_url = format!("{}{}", STEAM_API_BASE, "/ITwoFactorService/AddAuthenticator/v0001");
    let oauth_token = cached_data.oauth_token();
    let steamid = cached_data.steam_id().to_string();
    let time = steam_totp::time::Time::with_offset().await?.to_string();

    let payload = AddAuthenticatorRequest::new(oauth_token, &steamid, time.parse().unwrap());

    let response_text = client
        .request_with_session_guard(add_auth_url, Method::POST, None, Some(payload), None::<&str>)
        .and_then(|resp| resp.text().err_into())
        .await?;

    // The response carries the account secrets, so only its size is logged.
    debug!("Steam addauth response: {} bytes", response_text.len());

    let response = match serde_json::from_str::<AddAuthenticatorResponseBase>(&response_text) {
        Ok(resp) => resp.steam_guard_success_details,
        Err(err) => {
            debug!("AddAuthenticator response is not a success response: {err}");
            let error_resp = serde_json::from_str::<AddAuthenticatorErrorResponseBase>(&response_text)
                .map_err(InternalError::from)?;
            return match error_resp.response.status {
                29 => Err(LinkerError::AuthenticatorPresent),
                84 => Err(LinkerError::GeneralFailure(
                    "Steam is rate limiting us. Wait a couple of hours and try again.".to_string(),
                )),
                2 => Err(LinkerError::GeneralFailure(
                    "Steam refused to add the authenticator without saying why. The account may need a phone number, \
                     or this may be a temporary lock after too many attempts."
                        .to_string(),
                )),
                status => Err(LinkerError::GeneralFailure(format!(
                    "AddAuthenticator failed with status {status}."
                ))),
            };
        }
    };

    let mut mafile = response.mobile_auth;
    mafile.set_device_id(generate_canonical_device_id(&steamid));
    Ok(PendingAuthenticator {
        mafile,
        code_delivery: ActivationCodeDelivery::from_confirm_type(response.confirm_type, response.phone_number_hint),
    })
}

/// What the account falls back to after its authenticator is removed.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum RemoveAuthenticatorScheme {
    /// Keep SteamGuard, receiving codes by email.
    ReturnToEmailCodes,
    /// Disable SteamGuard entirely.
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
    cached_data: &SteamCache,
    revocation_token: &str,
    remove_authenticator_scheme: RemoveAuthenticatorScheme,
) -> LinkerResult<()> {
    let url = format!("{}{}", STEAM_API_BASE, "/ITwoFactorService/RemoveAuthenticator/v1");

    let steamid = cached_data.steam_id().to_string();
    let oauth_token = cached_data.oauth_token();
    let payload = RemoveAuthenticatorRequest::new(oauth_token, steamid, revocation_token, remove_authenticator_scheme);

    let response = client
        .request_with_session_guard_and_decode::<_, _, RemoveAuthenticatorResponseBase>(
            url,
            Method::POST,
            None,
            Some(payload),
            Some(cached_data.query_tokens()),
        )
        .await?
        .steam_guard_details;

    if !response.success {
        return Err(LinkerError::RemoveAuthenticatorFailed {
            attempts_remaining: response.revocation_attempts_remaining,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_delivery_from_confirm_type() {
        assert_eq!(
            ActivationCodeDelivery::from_confirm_type(Some(1), Some("12".to_string())),
            ActivationCodeDelivery::Sms {
                phone_hint: Some("12".to_string())
            }
        );
        assert_eq!(
            ActivationCodeDelivery::from_confirm_type(Some(1), Some(String::new())),
            ActivationCodeDelivery::Sms { phone_hint: None }
        );
        assert_eq!(
            ActivationCodeDelivery::from_confirm_type(Some(3), None),
            ActivationCodeDelivery::Email
        );
        assert_eq!(
            ActivationCodeDelivery::from_confirm_type(None, None),
            ActivationCodeDelivery::Unknown(0)
        );
    }
}
