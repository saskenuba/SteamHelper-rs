use crate::OfferId;
use crate::TRADEOFFER_BASE;
use crate::TRADEOFFER_NEW_URL;

pub mod context;
pub mod sessionid;
pub mod trade_link;
pub mod trade_offer_web;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TradeKind {
    Accept,
    Cancel,
    Create,
    Decline,
}

impl TradeKind {
    /// The URL of the operation; every one but `Create` is on an existing offer.
    pub fn endpoint(self, offer_id: Option<OfferId>) -> String {
        if let TradeKind::Create = self {
            return TRADEOFFER_NEW_URL.to_string();
        }

        let offer_id = offer_id.unwrap();
        let url_path = match self {
            Self::Accept => "/accept",
            Self::Cancel => "/cancel",
            Self::Decline => "/decline",
            Self::Create => unreachable!(),
        };
        TRADEOFFER_BASE.to_owned() + &*offer_id.to_string() + url_path
    }
}
