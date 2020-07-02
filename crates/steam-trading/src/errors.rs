use thiserror::Error;

#[derive(Error, Debug)]
pub enum TradeOfferError {
    #[error("The Tradeoffer URL was not valid.")]
    InvalidTradeOfferUrl,
    #[error("`{0}`")]
    InvalidTrade(String),
    #[error("`{0}`")]
    PayloadError(String),
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}
