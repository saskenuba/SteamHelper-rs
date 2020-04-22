//! The confirmation should be able to do the following operations:
//!
//! () Tell different authorizations apart. Like coming from the market or trades;
//! () Get all confirmations, so user can decide what to do with them;
//! () Autoconfirm authorizations based on N parameters;
//!
//!
use std::str::FromStr;

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};

// For trade offers confirmations.
// Maybe it can automatically accept with predetermined TradeOffersIDs , or coming from specific
// SteamIDs.
// For market confirmations it can maybe auto accept, but still with the option that the user
// retrieve all of them.

pub type ConfirmationType = EConfirmationType;

pub struct Confirmations(pub Vec<Confirmation>);

/// To retrieve a [Confirmation] we need to scrape the page
#[derive(Debug, Clone, PartialEq)]
pub struct Confirmation {
    pub id: String,
    pub key: String,
    pub kind: EConfirmationType,
    pub details: Option<ConfirmationDetails>,
}

/// We retrieve [ConfirmationDetails] as a json object.
/// There is also the need to already have a [Confirmation].
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmationDetails {
    /// ID of the trade offer. Has a value if EConfirmationType::Trade
    pub trade_offer_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, FromPrimitive)]
pub enum EConfirmationType {
    Unknown = 0,
    Generic = 1,
    Trade = 2,
    Market = 3,

    // We're missing information about definition of number 4 type
    PhoneNumberChange = 5,
    AccountRecovery = 6,
}

impl FromStr for EConfirmationType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let number = u32::from_str(s).unwrap();
        Ok(EConfirmationType::from_u32(number).unwrap())
    }
}

impl Confirmations {
    fn filter_by_confirmation(&mut self, confirmation_type: ConfirmationType) {
        self.0.retain(|confirmation| confirmation.kind == confirmation_type);
    }

    fn accept(&self) {}
}

impl From<Vec<Confirmation>> for Confirmations {
    fn from(confirmations_vec: Vec<Confirmation>) -> Self {
        Self { 0: confirmations_vec }
    }
}

enum EInventoryPrivacy {
    Unknown,
    Private,
    FriendsOnly,
    Public,
}
