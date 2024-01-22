//! Main error type of `SteamAuthenticator`.
//!
//! If an internal error occurs, it simply delegates to the correct error type.
//! Generally, this isn't a good strategy, but 90% of the errors happen because of a
//! misconfiguration, they are not recoverable and we choose to just fail fast.
//!
//!
//! For a general explanation of EResults, check: [steam errors website](https://steamerrors.com/).
use thiserror::Error;

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

    #[error(transparent)]
    HttpError(#[from] reqwest::Error),

    #[error(
        "A deserialization error has occurred. This indicates a change in the response or an unexpected situation. \
         Please report this issue."
    )]
    DeserializationError(#[from] serde_json::Error),
}
