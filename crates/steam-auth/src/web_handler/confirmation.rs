//! The confirmation should be able to do the following operations:
//!
//! () Tell different authorizations apart. Like coming from the market or trades;
//! () Get all confirmations, so user can decide what to do with them;
//! () Autoconfirm authorizations based on N parameters;
//!
//!
use serde::{Deserialize, Serialize};

// For trade offers confirmations.
// Maybe it can automatically accept with predetermined TradeOffersIDs , or coming from specific
// SteamIDs.
// For market confirmations it can maybe auto accept, but still with the option that the user
// retrieve all of them.

struct Confirmation {

}



#[derive(Debug, Clone, Serialize, Deserialize)]
enum EConfirmationType {
    Unknown = 0,
    Generic = 1,
    Trade = 2,
    Market = 3,

    // We're missing information about definition of number 4 type
    PhoneNumberChange = 5,
    AccountRecovery = 6,
}

fn get_web_confirmation_document() {}


enum EInventoryPrivacy {
    Unknown,
    Private,
    FriendsOnly,
    Public,
}