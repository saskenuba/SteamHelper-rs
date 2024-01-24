use std::convert::TryInto;

use tracing::info;

use crate::errors::OfferValidationError;
use crate::types::asset_collection::AssetCollection;
use crate::Tradelink;
use crate::TRADE_MAX_ITEMS;

#[derive(Debug, PartialEq)]
pub struct TradeOffer {
    /// The user who you want to trade with Steam Trade URL.
    pub their_tradelink: Tradelink,
    /// Assets you want to trade
    pub my_assets: Option<AssetCollection>,
    /// Assets you want from the other person.
    pub their_assets: Option<AssetCollection>,
    /// Optional trade offer message.
    pub message: String,
}

impl TradeOffer {
    pub fn new<MA, TA, S>(
        their_trade_url: String,
        my_assets: MA,
        their_assets: TA,
        message: S,
    ) -> Result<Self, OfferValidationError>
    where
        MA: Into<Option<AssetCollection>>,
        TA: Into<Option<AssetCollection>>,
        S: Into<Option<String>>,
    {
        let their_tradelink = their_trade_url.try_into()?;

        Ok(Self {
            their_tradelink,
            my_assets: my_assets.into(),
            their_assets: their_assets.into(),
            message: message.into().unwrap_or(String::new()),
        })
    }

    /// Validates if at least one item is being traded or if it exceeds the 255 items limit;
    pub fn validate(
        my_items: &Option<AssetCollection>,
        their_items: &Option<AssetCollection>,
    ) -> Result<(), OfferValidationError> {
        if my_items.is_none() && their_items.is_none() {
            return Err(OfferValidationError::InvalidTrade(
                "There can't be a trade offer with no items being traded.".to_string(),
            ));
        }

        // TODO: more elegant, please

        let my_length = my_items.as_ref().map(|c| c.0.len()).unwrap_or(0);
        let their_length = their_items.as_ref().map(|c| c.0.len()).unwrap_or(0);
        info!("Total items being traded: My: {} Their: {}", my_length, their_length);

        if my_length >= TRADE_MAX_ITEMS as usize || their_length >= TRADE_MAX_ITEMS as usize {
            return Err(OfferValidationError::InvalidTrade(format!(
                "Maximum number of items is: {}",
                TRADE_MAX_ITEMS
            )));
        }

        Ok(())
    }
}
