//! Types returned by Steam trade operations.
//!
//! The models described here can be found at:
//! `https://developer.valvesoftware.com/wiki/Steam_Web_API/IEconService`

use serde::Deserialize;
use serde_repr::Deserialize_repr;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use steam_language_gen::generated::enums::ETradeOfferConfirmationMethod;
use steam_language_gen::generated::enums::ETradeOfferState;

/// Tracks the status of a completed trade. I.e. after a trade offer has been accepted.
/// Received at GetTradeHistory endpoint, on `status` field.
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

#[serde_as]
#[derive(Deserialize, Debug, Clone)]
pub struct Descriptions {
    pub appid: u32,
    #[serde_as(as = "DisplayFromStr")]
    pub classid: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub instanceid: u32,
    pub marketable: bool,
    pub tradable: bool,
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
    pub response: CEcon_GetTradeOffers_Response,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
pub struct CEcon_GetTradeOffers_Response {
    pub trade_offers_sent: Option<Vec<TradeOffer_Trade>>,
    pub trade_offers_received: Option<Vec<TradeOffer_Trade>>,
}

#[serde_as]
#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
/// Represents a steam trade offer. CEcon_Trade
/// Returned by GetTradeOffers (vector) and GetTradeOffer.
pub struct TradeOffer_Trade {
    /// Unique ID generated when a trade offer is created
    #[serde_as(as = "DisplayFromStr")]
    pub tradeofferid: u64,
    /// SteamID3
    pub accountid_other: u64,
    /// Message included by the creator of the trade offer
    pub message: String,
    /// Unix time when the offer will expire (or expired, if it is in the past)
    expiration_time: i64,
    /// State of trade offer
    #[serde(rename = "trade_offer_state")]
    pub state: ETradeOfferState,
    items_to_give: Option<Vec<CEcon_Asset>>,
    items_to_receive: Option<Vec<CEcon_Asset>>,
    /// Indicates the account binded with the api key requested this trade
    pub is_our_offer: bool,
    pub time_created: i64,
    pub time_updated: i64,
    /// Historical number of the trade.
    /// Used find the new generated asset ids after the trade is completed.
    ///
    /// **Note**: Shows up only after the trade has been completed, and also can be found on the [TradeHistory_Trade]
    /// from the TradeHistory endpoint.
    pub tradeid: Option<String>,
    from_real_time_trade: bool,
    /// Unix timestamp of when the trade hold period is supposed to be over for this trade offer
    pub escrow_end_date: i64,
    confirmation_method: ETradeOfferConfirmationMethod,
}

#[serde_as]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CEcon_Asset {
    pub appid: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub contextid: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub assetid: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub classid: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub instanceid: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub amount: i64,
    pub missing: bool,
    #[serde_as(as = "DisplayFromStr")]
    pub est_usd: i64,
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

/// A trade returned by GetTradeHistory
/// Known as CEcon_GetTradeHistory_Response_Trade
#[allow(non_camel_case_types)]
#[serde_as]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TradeHistory_Trade {
    #[serde_as(as = "DisplayFromStr")]
    pub tradeid: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub steamid_other: u64,
    /// Unix epoch when the trade offer was completed, and turned into a trade.
    pub time_init: i64,
    pub time_escrow_end: Option<i64>,
    pub status: ETradeStatus,
    pub assets_received: Option<Vec<TradeHistory_TradedAsset>>,
    pub assets_given: Option<Vec<TradeHistory_TradedAsset>>,
}

#[serde_as]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
/// A traded item returned by GetTradeHistory
/// Known as  CEcon_GetTradeHistory_Response_Trade_TradedAsset
pub struct TradeHistory_TradedAsset {
    #[serde_as(as = "DisplayFromStr")]
    pub new_assetid: i64,
    pub rollback_new_assetid: Option<String>,
    #[serde_as(as = "DisplayFromStr")]
    pub new_contextid: u32,
    pub appid: u32,
    #[serde_as(as = "DisplayFromStr")]
    pub contextid: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub assetid: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub classid: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub instanceid: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub amount: i64,
    // #[serde(with = "DisplayFromStr")]
    // pub currencyid: Option<i64>,
}
