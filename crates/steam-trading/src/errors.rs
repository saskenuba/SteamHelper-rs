use thiserror::Error;

use steam_auth::errors::AuthError;
use steam_auth::HttpError;
use steam_language_gen::generated::enums::EResult;
use steam_web_api::errors::SteamAPIError;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TradeError {
    #[error("`{0}`")]
    PayloadError(String),

    #[error(transparent)]
    ConfirmationError(#[from] ConfirmationError),

    #[error(transparent)]
    TradeOfferError(#[from] OfferError),

    #[error(transparent)]
    SteamAPIError(#[from] SteamAPIError),

    #[error(transparent)]
    /// request errors
    HttpError(#[from] HttpError),

    /// inner steam authenticator errors
    #[error(transparent)]
    AuthError(#[from] AuthError),
}

#[derive(Error, Debug, PartialEq, Copy, Clone)]
///
pub enum TradelinkError {
    #[error("The Tradeoffer URL was not valid.")]
    Invalid,
}


#[derive(Error, Debug, PartialEq)]
///
pub enum OfferError {
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
    NotFoundButTradeCreated(i64),
}

pub(crate) fn tradeoffer_error_from_eresult(eresult: EResult) -> OfferError {
    match eresult {
        EResult::Revoked => OfferError::Revoked,
        EResult::InvalidState => OfferError::InvalidState,
        EResult::NoMatch => OfferError::NoMatch,
        e => OfferError::GeneralFailure(format!(
            "{}{}",
            "Please check: https://steamerrors.com/",
            &*serde_json::to_string(&e).unwrap()
        )),
    }
}

pub(crate) fn error_from_strmessage(message: &str) -> Option<OfferError> {
    let index_start = message.find(|c: char| c == '(')?;
    let index_end = message.find(|c: char| c == ')')?;

    let number = message
        .chars()
        .skip(index_start + 1)
        .take(index_end - index_start - 1)
        .collect::<String>();

    serde_json::from_str::<EResult>(&*number)
        .map(tradeoffer_error_from_eresult)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_strmessage() {
        let error_message = "Something went wrong (26)";
        assert_eq!(error_from_strmessage(error_message).unwrap(), OfferError::Revoked)
    }
}
