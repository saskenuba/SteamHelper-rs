use thiserror::Error;

use steam_auth::errors::AuthError;
use steam_auth::HttpError;
use steam_language_gen::generated::enums::EResult;

#[derive(Error, Debug)]
pub enum TradeError {
    #[error("`{0}`")]
    PayloadError(String),

    #[error(transparent)]
    ConfirmationError(#[from] ConfirmationError),

    #[error(transparent)]
    TradeOfferError(#[from] TradeOfferError),

    #[error(transparent)]
    /// request errors
    HttpError(#[from] HttpError),

    /// inner steam authenticator errors
    #[error(transparent)]
    AuthError(#[from] AuthError),
}

#[derive(Error, Debug)]
///
pub enum TradeOfferError {
    #[error("The Tradeoffer URL was not valid.")]
    InvalidTradeOfferUrl,
    #[error("`{0}`")]
    InvalidTrade(String),

    #[error("This trade offer is in an invalid state, and cannot be acted upon. Perhaps you are trying to cancel a trade offer \
    that was already canceled, or something similar.")]
    InvalidState,

    #[error("This trade offer if could not be found. Are you sure this is the correct id?")]
    NoMatch,

    #[error("This response code suggests that one or more of the items in this trade offer does not exist in the inventory from which \
    it was requested.")]
    Revoked,

    #[error("General Failure: `{0}`")]
    GeneralFailure(String),
}

#[derive(Error, Debug, Copy, Clone)]
pub enum ConfirmationError {
    #[error("Could not find the requested confirmation.")]
    NotFound,
    #[error("Could not find the requested confirmation, but offer was created. Trade offer id: `{0}`")]
    NotFoundButTradeCreated(u64),
}

pub(crate) fn tradeoffer_error_from_eresult(eresult: EResult) -> TradeOfferError {
    match eresult {
        EResult::Revoked => TradeOfferError::Revoked,
        EResult::InvalidState => TradeOfferError::InvalidState,
        EResult::NoMatch => TradeOfferError::NoMatch,
        e => TradeOfferError::GeneralFailure(format!(
            "{}{}",
            "Please check: https://steamerrors.com/",
            &*serde_json::to_string(&e).unwrap()
        )),
    }
}

/*pub(crate) fn error_from_strmessage(message: &str) -> Option<TradeOfferError> {
    let index_start = message.find(|c: char| c == '(')?;
    let index_end = message.find(|c: char| c == ')')?;

    message[index_start + 1].to_owned() + &message[..index_end - 1]
}*/
