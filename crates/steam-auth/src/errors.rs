use thiserror::Error;

/// Main error type that SteamAuthenticator uses.
///
/// If an internal error occurs, it simply delegates to the correct error type.
/// Generally, this isn't a good strategy, but 90% of the errors happen because of a
/// misconfiguration, they are not recoverable and we choose to just fail fast.
///
///
/// For a general explanation of EResults, check: https://steamerrors.com/
#[derive(Error, Debug)]
pub enum AuthError {
    #[error(transparent)]
    ApiKeyError(#[from] ApiKeyError),
    #[error(transparent)]
    Login(#[from] LoginError),
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}

#[derive(Error, Debug)]
pub enum ApiKeyError {
    #[error("General Failure: `{0}`")]
    GeneralError(String),
    #[error("This key is unavailable for registering.")]
    AccessDenied,
    #[error("Key not yet registered.")]
    NotRegistered,
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}

#[derive(Error, Debug)]
pub enum LoginError {
    #[error("`{0}`")]
    GeneralFailure(String),
    #[error("Need a SteamID associated with user.")]
    NeedSteamID,
    #[error("Parental unlock error `{0}`")]
    ParentalUnlock(String),
    #[error("Requires a captcha code.")]
    CaptchaCode,
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}

/// Errors related to the Authenticator Linker.
#[derive(Error, Debug)]
pub enum LinkerError {
    #[error("{0}")]
    GeneralFailure(String),
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}
