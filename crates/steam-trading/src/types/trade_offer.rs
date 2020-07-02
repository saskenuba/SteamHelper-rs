use crate::types::asset_collection::AssetCollection;

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
}
