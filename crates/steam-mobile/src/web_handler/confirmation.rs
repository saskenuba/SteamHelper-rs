use std::fmt::Display;
use std::fmt::Formatter;
use std::iter::FromIterator;
use std::str::FromStr;

use derive_more::Deref;
use derive_more::IntoIterator;
use serde::Deserialize;
use serde::Serialize;
use serde_repr::Deserialize_repr;
use serde_repr::Serialize_repr;

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
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Confirmation {
    pub id: String,
    #[serde(rename = "nonce")]
    pub key: String,
    #[serde(rename = "type")]
    pub kind: EConfirmationType,
    pub creation_time: i64,
    pub creator_id: String,
    pub type_name: String,
    // from below here, nothing really useful
    // pub cancel: String,
    // pub accept: String,
    // pub icon: String,
    // pub multi: bool,
    // pub headline: String,
    // pub summary: Vec<String>,
    // pub warn: Option<String>,
}

impl Display for Confirmation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Confirmation {} of {:?}", self.key, self.kind)
    }
}

impl Confirmation {
    pub fn has_trade_offer_id(&self, offer_id: i64) -> bool {
        if self.kind == EConfirmationType::Trade {
            // return self.details.map_or(false, |d| d.trade_offer_id == Some(offer_id));
            todo!()
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

#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, Eq, PartialEq)]
#[repr(u8)]
#[non_exhaustive]
pub enum EConfirmationType {
    /// Unknown confirmation
    Unknown = 0,
    /// Under rare circumstances this might pop up
    Generic = 1,
    /// Confirmation from Trade Offer
    Trade = 2,
    /// Confirmation from Steam's Market
    Market = 3,

    /// Unknown
    FeatureOptOut = 4,
    /// Confirmation for a phone number change
    PhoneNumberChange = 5,
    /// Confirmation for account recovery
    AccountRecovery = 6,
    /// Confirmation for creating a new API Key,
    APIKey = 9,
}

impl From<Vec<Confirmation>> for Confirmations {
    fn from(confirmations_vec: Vec<Confirmation>) -> Self {
        Self(confirmations_vec)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ConfirmationTag {
    Confirmation,
    ConfirmationDetails,
}

impl ConfirmationTag {
    pub(crate) const fn value(&self) -> &'static str {
        match *self {
            ConfirmationTag::Confirmation => "conf",
            ConfirmationTag::ConfirmationDetails => "details",
        }
    }
}

#[allow(missing_docs)]
#[derive(Copy, Clone, Debug)]
pub enum ConfirmationMethod {
    Accept,
    Deny,
}

impl ConfirmationMethod {
    pub(crate) const fn value(&self) -> &'static str {
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
