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
    AuthenticatorError(#[from] LinkerError),
    #[error(transparent)]
    ApiKeyError(#[from] ApiKeyError),
    #[error(transparent)]
    Login(#[from] LoginError),
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}

#[derive(Error, Debug)]
/// This kind of error should only be raised, if the user tried to use a method that requires the API KEY, but it could
/// not be cached for any reason.
pub enum ApiKeyError {
    #[error("General Failure: `{0}`")]
    GeneralError(String),
    #[error("This key is unavailable for registering.")]
    AccessDenied,
    #[error("Key not yet registered.")]
    NotRegistered,

    #[error("A method requiring a cached key was used, but this account API KEY could not be cached.")]
    NotCached,
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}

#[derive(Error, Debug)]
pub enum LoginError {
    #[error("Message returned: `{0}`")]
    GeneralFailure(String),
    #[error("Need a SteamID associated with user.")]
    NeedSteamID,
    #[error("Parental unlock error `{0}`")]
    ParentalUnlock(String),
    #[error("Account name or password entered are incorrect.")]
    IncorrectCredentials,
    #[error("Requires a captcha code. If a previous attempt was made, the captcha was probably incorrect. \
    Captcha GUID: `{0}`", .captcha_guid)]
    CaptchaRequired { captcha_guid: String },
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}

/// Errors related to the Authenticator Linker.
#[derive(Error, Debug)]
pub enum LinkerError {
    #[error("{0}")]
    GeneralFailure(String),
    #[error("There is already a authenticator vinculated with this account. Remove the old to add another one.")]
    /// There is already a finalized authenticator on this account. If you want to add on another number, first remove
    /// the old one.
    AuthenticatorPresent,

    #[error("The SMS code you entered is incorrect.")]
    BadSMSCode,
    #[error("We were unable to generate the correct codes. Perhaps something changed?")]
    UnableToGenerateCorrectCodes,

    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
    #[error(transparent)]
    TotpError(#[from] steam_totp::error::TotpError),
}
