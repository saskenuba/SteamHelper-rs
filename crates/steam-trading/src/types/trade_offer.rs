use steam_auth::Url;
use tracing::info;

use crate::errors::OfferError;
use crate::types::asset_collection::AssetCollection;
use crate::TRADE_MAX_ITEMS;

#[derive(Debug, PartialEq)]
pub struct TradeOffer {
    /// The user who you want to trade with Steam Trade URL.
    pub their_trade_url: String,
    /// Assets you want to trade
    pub my_assets: Option<AssetCollection>,
    /// Assets you want from the other person.
    pub their_assets: Option<AssetCollection>,
    /// Optional trade offer message.
    pub message: String,
}

impl TradeOffer {
    pub fn new<MA, TA, S>(their_trade_url: String, my_assets: MA, their_assets: TA, message: S) -> Self
    where
        MA: Into<Option<AssetCollection>>,
        TA: Into<Option<AssetCollection>>,
        S: Into<Option<String>>,
    {
        Self {
            their_trade_url,
            my_assets: my_assets.into(),
            their_assets: their_assets.into(),
            message: message.into().unwrap_or_else(|| "".to_string()),
        }
    }

    /// Validates if at least one item is being traded or if it exceeds the 255 items limit;
    pub fn validate(
        my_items: &Option<AssetCollection>,
        their_items: &Option<AssetCollection>,
    ) -> Result<(), OfferError> {
        if my_items.is_none() && their_items.is_none() {
            return Err(OfferError::InvalidTrade(
                "There can't be a trade offer with no items being traded.".to_string(),
            ));
        }

        // TODO: more elegant, please

        let my_length = my_items.as_ref().map(|c| c.0.len()).unwrap_or(0);
        let their_length = their_items.as_ref().map(|c| c.0.len()).unwrap_or(0);
        info!("Total items being traded: My: {} Their: {}", my_length, their_length);

        if my_length >= TRADE_MAX_ITEMS as usize || their_length >= TRADE_MAX_ITEMS as usize {
            return Err(OfferError::InvalidTrade(format!(
                "Maximum number of items is: {}",
                TRADE_MAX_ITEMS
            )));
        }

        Ok(())
    }

    pub(crate) fn parse_url(url: &str) -> Result<(String, Option<String>), OfferError> {
        let parsed_url = Url::parse(url).unwrap();

        // Partner ID is mandatory
        let steam_id3 = parsed_url
            .query_pairs()
            .find(|(param, _)| param == "partner")
            .ok_or_else(|| OfferError::InvalidTradeOfferUrl)?
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
