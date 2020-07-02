//! Placeholder for trade crate?

use serde::{Deserialize, Serialize};

/// This is decided upon various factors, mainly stability of Steam servers when dealing with huge
/// trade offers Consider this when creating trade websites.
const TRADE_MAX_ITEMS: u8 = u8::max_value();

/// Limit introduced by Valve
const TRADE_MAX_TRADES_PER_ACCOUNT: u8 = 5;

/// Parameter of GetTradeOffers
enum TradeOfferStatus {
    Active,
    Historical,
    All,
}

impl TradeOfferStatus {
    pub fn value(&self) -> &'static str {
        match *self {
            TradeOfferStatus::Active => "&active_only=1",
            TradeOfferStatus::Historical => "&historical_only=1",
            TradeOfferStatus::All => "&active_only=1&historical_only=1",
        }
    }
}

/// Parameter of GetTradeOffers
enum TradeOfferTime {
    Sent,
    Received,
    All,
}

impl TradeOfferTime {
    pub fn value(&self) -> &'static str {
        match *self {
            TradeOfferTime::Sent => "&get_sent_offers=1",
            TradeOfferTime::Received => "&get_received_offers=1",
            TradeOfferTime::All => "&get_sent_offers=1&get_received_offers=1",
        }
    }
}

/// Tracks the status of a trade after a trade offer has been accepted.
/// Received at GetTradeHistory at status field on
/// CEcon_GetTradeHistory_Response_Trade
enum ETradeStatus {
    Init = 0,         // Trade has just been accepted/confirmed, but no work has been done yet
    PreCommitted = 1, // Steam is about to start committing the trade
    Committed = 2,    // The items have been exchanged
    Complete = 3,     // All work is finished
    Failed = 4,       /* Something went wrong after Init, but before Committed, and the trade
                       * has been rolled back */
    PartialSupportRollback = 5, // A support person rolled back the trade for one side
    FullSupportRollback = 6,    // A support person rolled back the trade for both sides
    SupportRollbackSelective = 7, // A support person rolled back the trade for some set of items
    RollbackFailed = 8,         /* We tried to roll back the trade when it failed, but haven't
                                 * managed to do that for all items yet */
    RollbackAbandoned = 9, /* We tried to roll back the trade, but some failure didn't go away
                            * and we gave up */
    InEscrow = 10,       // Trade is in escrow
    EscrowRollback = 11, // A trade in escrow was rolled back
}

#[derive(Serialize)]
struct TradeOfferAcceptRequest<'a> {
    pub server_id: &'a str,
    #[serde(rename = "tradeofferid")]
    pub trade_offer_id: &'a str,
}

impl<'a> Default for TradeOfferAcceptRequest<'a> {
    fn default() -> Self {
        Self {
            server_id: "1",
            trade_offer_id: "",
        }
    }
}
