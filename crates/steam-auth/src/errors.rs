use thiserror::Error;

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
