use crate::{errors::TradeOfferError, types::asset_collection::AssetCollection};
use reqwest::Url;

#[derive(Debug, PartialEq)]
pub struct TradeOffer {
    pub url: String,
    pub my_assets: Option<AssetCollection>,
    pub their_assets: Option<AssetCollection>,
    pub message: String,
}

impl TradeOffer {
    pub fn new(
        url: String,
        my_assets: Option<AssetCollection>,
        their_assets: Option<AssetCollection>,
        message: String,
    ) -> Self {
        Self {
            url,
            my_assets,
            their_assets,
            message,
        }
    }

    pub fn validate(
        my_items: &Option<AssetCollection>,
        their_items: &Option<AssetCollection>,
    ) -> Result<(), TradeOfferError> {
        if my_items.is_none() && their_items.is_none() {
            return Err(TradeOfferError::InvalidTrade(
                "There can't be a trade offer with no items being traded.".to_string(),
            ));
        }

        Ok(())
    }

    pub(crate) fn parse_url(url: &str) -> Result<(String, Option<String>), TradeOfferError> {
        let parsed_url = Url::parse(url).unwrap();

        // Partner ID is mandatory
        let steam_id3 = parsed_url
            .query_pairs()
            .find(|(param, _)| param == "partner")
            .ok_or_else(|| TradeOfferError::InvalidTradeOfferUrl)?
            .1
            .to_string();

        // If the recipient is your friend, you don't need a token
        let trade_token = parsed_url
            .query_pairs()
            .find(|(param, _)| param == "token")
            .map(|(_, c)| c.to_string());

        Ok((steam_id3, trade_token))
    }
}
