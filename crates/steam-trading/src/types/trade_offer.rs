use crate::{errors::TradeOfferError, types::asset_collection::AssetCollection, TRADE_MAX_ITEMS};
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

    /// Validates if at least one item is being traded or if it exceeds the 255 items limit;
    pub fn validate(
        my_items: &Option<AssetCollection>,
        their_items: &Option<AssetCollection>,
    ) -> Result<(), TradeOfferError> {
        if my_items.is_none() && their_items.is_none() {
            return Err(TradeOfferError::InvalidTrade(
                "There can't be a trade offer with no items being traded.".to_string(),
            ));
        }

        // TODO: more elegant, please

        let my_length = my_items.as_ref().map(|c| c.0.len()).unwrap();
        let their_length = their_items.as_ref().map(|c| c.0.len()).unwrap();

        if my_length >= TRADE_MAX_ITEMS as usize || their_length >= TRADE_MAX_ITEMS as usize {
            return Err(TradeOfferError::InvalidTrade(format!(
                "Maximum number of items is: {}",
                TRADE_MAX_ITEMS
            )));
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
