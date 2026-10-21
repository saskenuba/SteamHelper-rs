//! Main error type of `SteamAuthenticator`.
//!
//! If an internal error occurs, it simply delegates to the correct error type.
//! Generally, this isn't a good strategy, but 90% of the errors happen because of a
//! misconfiguration, they are not recoverable and we choose to just fail fast.
//!
//!
//! For a general explanation of EResults, check: [steam errors website](https://steamerrors.com/).
use thiserror::Error;

use crate::types::eresult_from_i32;

#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum AuthError {
    #[error(transparent)]
    AuthenticatorError(#[from] LinkerError),
    #[error(transparent)]
    ApiKeyError(#[from] ApiKeyError),
    #[error(transparent)]
    Login(#[from] LoginError),
    #[error(transparent)]
    MobileAuthFile(#[from] MobileAuthFileError),
    #[error(transparent)]
    InternalError(#[from] InternalError),
}

#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum ConfirmationError {
    #[error("General Failure: `{0}`")]
    GeneralError(String),
}

#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum ApiKeyError {
    #[error("General Failure: `{0}`")]
    GeneralError(String),
    #[error("This key is unavailable for registering.")]
    AccessDenied,
    #[error("Key not yet registered.")]
    NotRegistered,
    #[error("Could not find the mobile confirmation for this API key request.")]
    ConfirmationNotFound,
    #[error(transparent)]
    InternalError(#[from] InternalError),
}

#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum LoginError {
    #[error("Message returned: `{0}`")]
    GeneralFailure(String),
    #[error("Parental unlock error `{0}`")]
    ParentalUnlock(String),
    #[error("Steam Guard Mobile is not enabled. Email codes are not supported.")]
    Need2FA,
    #[error("Account name or password entered are incorrect.")]
    IncorrectCredentials,
    #[error("Requires a captcha code. If a previous attempt was made, the captcha was probably incorrect. \
    Captcha GUID: `{0}`", .captcha_guid)]
    CaptchaRequired { captcha_guid: String },
    #[error(transparent)]
    InternalError(#[from] InternalError),
    #[error(transparent)]
    TotpError(#[from] steam_totp::error::TotpError),
}

/// Errors related to the Authenticator Linker.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum LinkerError {
    #[error("{0}")]
    GeneralFailure(String),
    #[error("An authenticator is already linked to this account. Please remove the old one before adding a new one.")]
    AuthenticatorPresent,
    #[error("The SMS code you entered is incorrect.")]
    BadSMSCode,
    #[error("We were unable to generate the correct codes. Perhaps something changed?")]
    UnableToGenerateCorrectCodes,
    #[error("Steam rejected the phone number.")]
    PhoneNumberRejected,
    #[error("The email sent by Steam to allow adding a phone number was not confirmed yet.")]
    EmailNotConfirmed,
    #[error("Failed to remove the authenticator. Revocation attempts remaining: {attempts_remaining}")]
    RemoveAuthenticatorFailed { attempts_remaining: i64 },
    #[error(transparent)]
    InternalError(#[from] InternalError),
    #[error(transparent)]
    TotpError(#[from] steam_totp::error::TotpError),
}

/// Errors related to the Authenticator Linker.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum MobileAuthFileError {
    #[error(transparent)]
    InternalError(#[from] InternalError),

    #[error("{0}")]
    GeneralFailure(String),
}

/// Errors from networking or failure to deserialize internal types.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum InternalError {
    #[error("`{0}`")]
    GeneralFailure(String),

    /// Its URL has no query string, which may carry tokens or confirmation hashes.
    #[error(transparent)]
    HttpError(reqwest::Error),

    #[error("The web session expired. Log in again, or renew the session.")]
    SessionExpired,

    /// The Steam client of a login through it (feature `cm`, `SteamAuthenticator::login_with_client`) failed to issue
    /// the session an access token: at the login, or before a request once the session's token has expired, which
    /// then wasn't sent (while the token is still valid, a failed renewal lets the request go with it). Within 30
    /// seconds of a failed renewal the client isn't asked again, and this is the error it failed with then.
    ///
    /// The source is the client's error shared behind an `Arc`, at the login as after a renewal: downcast it to
    /// `Arc<steam_client::errors::ClientError>`, such as `Closed` once the client stopped, or `InvalidToken` for a
    /// token whose expiry is out of range. Every request failing with the same renewal's error gets the same one.
    #[error("The Steam client failed to issue the web session an access token.")]
    ClientToken(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// Steam answered HTTP 429, as its firewall does to requests it suspects come from a bot. Retry later, and more slowly.
    #[error("Steam rate limited the request to {url} (HTTP 429 Too Many Requests).")]
    TooManyRequests { url: String },

    /// Steam answered another HTTP error status. `body` is the start of its error page, whitespace collapsed.
    #[error("Steam answered HTTP {status} to {url}: `{body}`")]
    HttpStatus {
        status: reqwest::StatusCode,
        url: String,
        body: String,
    },

    /// Steam's Web API reported a failure in the `x-eresult` header. `eresult` is the raw number, since Steam may
    /// send one [`EResult`](steam_language::enums::EResult) doesn't know yet.
    #[error("Steam answered {url} with EResult {eresult} ({}).", eresult_name(*.eresult))]
    EResult { eresult: i32, url: String },

    #[error(
        "A deserialization error has occurred. This indicates a change in the response or an unexpected situation. \
         Please report this issue."
    )]
    DeserializationError(#[from] serde_json::Error),
}

/// `reqwest::Error` shows its URL in `Display` and `Debug`: the query is removed, as [`InternalError`]'s other URLs.
impl From<reqwest::Error> for InternalError {
    fn from(mut error: reqwest::Error) -> Self {
        if let Some(url) = error.url_mut() {
            url.set_query(None);
        }
        InternalError::HttpError(error)
    }
}

fn eresult_name(eresult: i32) -> String {
    eresult_from_i32(eresult).map_or_else(|| "unknown".to_string(), |known| format!("{known:?}"))
}
