use std::fmt::Display;
use std::fmt::Formatter;
use std::iter::FromIterator;

use derive_more::Deref;
use derive_more::IntoIterator;
use serde::Deserialize;
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
    pub fn has_trade_offer_id(&self, offer_id: u64) -> bool {
        self.trade_offer_id() == Some(offer_id)
    }
    pub fn trade_offer_id(&self) -> Option<u64> {
        if self.kind == EConfirmationType::Trade {
            self.creator_id.parse().ok()
        } else {
            None
        }
    }
}

/// We retrieve [`ConfirmationDetails`] as a json object.
/// There is also the need to already have a [Confirmation].
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub struct ConfirmationDetails {
    /// ID of the trade offer. Has a value if EConfirmationType::Trade
    pub trade_offer_id: Option<i64>,
}

/// Kinds of mobile confirmations, mirroring Steam's `EMobileConfirmationType`.
#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, Eq, PartialEq)]
#[repr(u8)]
#[non_exhaustive]
pub enum EConfirmationType {
    /// Unknown confirmation. Kinds Steam adds later also land here, instead of failing the whole fetch; the
    /// confirmation's `type_name` still says what it is.
    #[serde(other)]
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
    /// Unknown
    BuildChangeRequest = 7,
    /// Unknown
    AddUser = 8,
    /// Confirmation for creating a new API Key,
    APIKey = 9,
    /// Confirmation for inviting someone to your Steam Family group
    InviteToFamilyGroup = 10,
    /// Confirmation for joining a Steam Family group
    JoinFamilyGroup = 11,
    /// Confirmation for buying from Steam's Market
    MarketPurchase = 12,
    /// Confirmation for a refund request
    RequestRefund = 13,
}

impl From<Vec<Confirmation>> for Confirmations {
    fn from(confirmations_vec: Vec<Confirmation>) -> Self {
        Self(confirmations_vec)
    }
}

#[allow(missing_docs)]
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum ConfirmationAction {
    Retrieve,
    Accept,
    Deny,
}

impl ConfirmationAction {
    pub(crate) const fn as_operation(self) -> Option<&'static str> {
        Some(match self {
            Self::Accept => "allow",
            Self::Deny => "cancel",
            _ => return None,
        })
    }
    pub(crate) const fn as_tag(self) -> &'static str {
        "conf"
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

    fn confirmation(id: &str, kind: EConfirmationType, creator_id: &str) -> Confirmation {
        Confirmation {
            id: id.to_string(),
            key: "18064583892738866189".to_string(),
            kind,
            creation_time: 0,
            creator_id: creator_id.to_string(),
            type_name: String::new(),
        }
    }

    #[test]
    fn trade_offer_id_only_for_trades() {
        let trade = confirmation("7676451136", EConfirmationType::Trade, "4009687284");
        let market = confirmation("7652515663", EConfirmationType::Market, "4000980011");

        assert_eq!(trade.trade_offer_id(), Some(4009687284));
        assert!(trade.has_trade_offer_id(4009687284));
        assert_eq!(market.trade_offer_id(), None);
        assert!(!market.has_trade_offer_id(4000980011));
    }

    #[test]
    fn unmapped_kinds_do_not_fail_parsing() {
        let json = r#"[
            {"id":"1","nonce":"2","type":11,"creation_time":0,"creator_id":"3","type_name":"Join Steam Family"},
            {"id":"4","nonce":"5","type":200,"creation_time":0,"creator_id":"6","type_name":"Something new"}
        ]"#;
        let confirmations: Vec<Confirmation> = serde_json::from_str(json).unwrap();

        assert_eq!(confirmations[0].kind, EConfirmationType::JoinFamilyGroup);
        assert_eq!(confirmations[1].kind, EConfirmationType::Unknown);
        assert_eq!(confirmations[1].type_name, "Something new");
    }
}
