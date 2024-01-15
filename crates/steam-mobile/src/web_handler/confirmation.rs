use std::iter::FromIterator;
use std::str::FromStr;

use derive_more::Deref;
use derive_more::IntoIterator;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use serde::Deserialize;
use serde::Serialize;

/// A collection of [`Confirmation`]
#[derive(IntoIterator, Deref, Default, Debug)]
pub struct Confirmations(#[into_iterator(owned, ref)] pub Vec<Confirmation>);

impl<'a> FromIterator<&'a Confirmation> for Confirmations {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = &'a Confirmation>,
    {
        let buffer = iter.into_iter().cloned().collect::<Vec<_>>();
        Self(buffer)
    }
}

/// A pending Steam confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmation {
    pub id: String,
    pub key: String,
    pub kind: EConfirmationType,
    pub details: Option<ConfirmationDetails>,
}

impl Confirmation {
    pub fn has_trade_offer_id(&self, offer_id: i64) -> bool {
        if self.kind == EConfirmationType::Trade {
            return self.details.map_or(false, |d| d.trade_offer_id == Some(offer_id));
        }
        false
    }
}

/// We retrieve [`ConfirmationDetails`] as a json object.
/// There is also the need to already have a [Confirmation].
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub struct ConfirmationDetails {
    /// ID of the trade offer. Has a value if EConfirmationType::Trade
    pub trade_offer_id: Option<i64>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq, FromPrimitive)]
/// Kinds of confirmations that exist.
pub enum EConfirmationType {
    /// Unknown confirmation
    Unknown = 0,
    /// Under rare circumstances this might pop up
    Generic = 1,
    /// Confirmation from Trade Offer
    Trade = 2,
    /// Confirmation from Steam's Market
    Market = 3,

    // We're missing information about definition of number 4 type
    /// Confirmation for a phone number change
    PhoneNumberChange = 5,
    /// Confirmation for account recovery
    AccountRecovery = 6,
}

impl FromStr for EConfirmationType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let number = u32::from_str(s).unwrap();
        Ok(EConfirmationType::from_u32(number).unwrap())
    }
}

impl From<Vec<Confirmation>> for Confirmations {
    fn from(confirmations_vec: Vec<Confirmation>) -> Self {
        Self { 0: confirmations_vec }
    }
}

#[allow(missing_docs)]
#[derive(Copy, Clone, Debug)]
pub enum ConfirmationMethod {
    Accept,
    Deny,
}

impl ConfirmationMethod {
    pub(crate) fn value(&self) -> &'static str {
        match *self {
            Self::Accept => "allow",
            Self::Deny => "cancel",
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum EInventoryPrivacy {
    Unknown,
    Private,
    FriendsOnly,
    Public,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_confirmations() -> Confirmations {
        let confirmations = vec![
            Confirmation {
                id: "7676451136".to_string(),
                key: "18064583892738866189".to_string(),
                kind: EConfirmationType::Trade,
                details: Some(ConfirmationDetails {
                    trade_offer_id: Some(4009687284),
                }),
            },
            Confirmation {
                id: "7652515663".to_string(),
                key: "10704556181383316145".to_string(),
                kind: EConfirmationType::Trade,
                details: Some(ConfirmationDetails {
                    trade_offer_id: Some(4000980011),
                }),
            },
            Confirmation {
                id: "7652555421".to_string(),
                key: "10704556181383323456".to_string(),
                kind: EConfirmationType::Trade,
                details: Some(ConfirmationDetails {
                    trade_offer_id: Some(4000793103),
                }),
            },
            Confirmation {
                id: "7652515663".to_string(),
                key: "20845677815483316145".to_string(),
                kind: EConfirmationType::Market,
                details: None,
            },
        ];
        Confirmations::from(confirmations)
    }
}
