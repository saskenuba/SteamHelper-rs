//! Types returned by Steam trade operations.
//!
//! The models described here can be found at:
//! https://developer.valvesoftware.com/wiki/Steam_Web_API/IEconService

use serde::Deserialize;
use serde_repr::Deserialize_repr;

use steam_language_gen::generated::enums::{ETradeOfferConfirmationMethod, ETradeOfferState};

/// Tracks the status of a completed trade. I.e. after a trade offer has been accepted.
/// Received at GetTradeHistory at status field on CEcon_GetTradeHistory_Response_Trade
#[derive(Debug, Clone, PartialEq, Eq, Deserialize_repr)]
#[repr(i32)]
pub enum ETradeStatus {
    /// Trade has just been accepted/confirmed, but no work has been done yet
    Init = 0,
    /// Steam is about to start committing the trade
    PreCommitted = 1,
    /// The items have been exchanged
    Committed = 2,
    /// All work is finished
    Complete = 3,
    /// Something went wrong after Init, but before Committed, and the trade has been rolled back
    Failed = 4,
    /// A support person rolled back the trade for one side
    PartialSupportRollback = 5,
    /// A support person rolled back the trade for both sides
    FullSupportRollback = 6,
    /// A support person rolled back the trade for some set of items
    SupportRollbackSelective = 7,
    /// We tried to roll back the trade when it failed, but haven't managed to do that for all
    /// items yet
    RollbackFailed = 8,
    /// We tried to roll back the trade, but some failure didn't go away and we gave up */
    RollbackAbandoned = 9,
    /// Trade is in escrow
    InEscrow = 10,
    /// A trade in escrow was rolled back
    EscrowRollback = 11,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Descriptions {
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub appid: u32,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub classid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub instanceid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub marketable: bool,
    pub tradable: String,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
pub struct GetTradeOfferResponse {
    pub response: CEcon_GetTradeOffer_Response,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
pub struct CEcon_GetTradeOffer_Response {
    pub offer: TradeOffer_Trade,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
/// Represents the raw CEcon_GetTradeOffers_Response_Base
pub struct GetTradeOffersResponse {
    pub(crate) response: CEcon_GetTradeOffers_Response,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
pub struct CEcon_GetTradeOffers_Response {
    pub trade_offers_sent: Option<Vec<TradeOffer_Trade>>,
    pub trade_offers_received: Option<Vec<TradeOffer_Trade>>,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
/// Represents a steam trade offer. CEcon_Trade
/// Returned by GetTradeOffers (vector) and GetTradeOffer.
pub struct TradeOffer_Trade {
    /// Unique ID generated when a trade offer is created
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub tradeofferid: u64,
    /// SteamID3
    pub accountid_other: u64,
    /// Message included by the creator of the trade offer
    message: String,
    /// Unix time when the offer will expire (or expired, if it is in the past)
    expiration_time: u64,
    /// State of trade offer
    #[serde(rename = "trade_offer_state")]
    pub state: ETradeOfferState,
    items_to_give: Option<Vec<CEcon_Asset>>,
    items_to_receive: Option<Vec<CEcon_Asset>>,
    /// Indicates the account binded with the api key requested this trade
    pub is_our_offer: bool,
    time_created: i64,
    time_updated: i64,
    /// Tradeid is the historical number of the trade.
    /// It is used, for example to find the new generated asset ids after the trade is completed.
    ///
    /// Shows up only after the trade has been completed, and also can be found on the TradeHistory endpoint.
    tradeid: Option<String>,
    from_real_time_trade: bool,
    /// Unix timestamp of when the trade hold period is supposed to be over for this trade offer
    escrow_end_date: i64,
    confirmation_method: ETradeOfferConfirmationMethod,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CEcon_Asset {
    #[serde(flatten)]
    pub asset: AssetFields,
    pub missing: bool,
    pub est_usd: String,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
/// Represents the raw CEcon_GetTradeHistory_Response_Trade_Base
pub struct GetTradeHistoryResponse {
    pub response: CEcon_GetTradeHistory_Response_Trade_Intermediate,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CEcon_GetTradeHistory_Response_Trade_Intermediate {
    pub more: bool,
    pub trades: Vec<TradeHistory_Trade>,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
/// A trade returned by GetTradeHistory
/// Known as CEcon_GetTradeHistory_Response_Trade
pub struct TradeHistory_Trade {
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub tradeid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub steamid_other: u64,
    /// Unix epoch when the trade offer was completed, and turned into a trade.
    pub time_init: i64,
    pub status: ETradeStatus,
    pub assets_received: Option<Vec<TradeHistory_TradedAsset>>,
    pub assets_given: Option<Vec<TradeHistory_TradedAsset>>,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
/// A traded item returned by GetTradeHistory
/// Known as  CEcon_GetTradeHistory_Response_Trade_TradedAsset
pub struct TradeHistory_TradedAsset {
    #[serde(flatten)]
    pub assetids: AssetIDHistory,
    #[serde(flatten)]
    pub asset: AssetFields,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub new_contextid: u64,
    pub rollback_new_contextid: Option<String>,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AssetIDHistory {
    #[serde(with = "serde_with::rust::display_fromstr")]
    #[serde(rename = "assetid")]
    pub old_assetid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub new_assetid: u64,
    pub rollback_new_assetid: Option<String>,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Deserialize)]
pub struct AssetFields {
    pub appid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub contextid: u64,
    // #[serde(with = "serde_with::rust::display_fromstr")]
    // pub currencyid: Option<u64>,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub classid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub instanceid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub amount: u64,
}
