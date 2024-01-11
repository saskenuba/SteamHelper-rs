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

impl Confirmations {
    /// Convenience function that filters confirmations based directly on [`EConfirmationType`].
    ///
    /// # Example
    /// ```no_run
    /// use steam_mobile::ConfirmationMethod;
    /// use steam_mobile::EConfirmationType;
    /// use steam_mobile::User;
    /// # use steam_mobile::SteamAuthenticator;
    /// # use steam_mobile::Confirmations;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// # let user = User::new("username".to_string(), "password".to_string());
    /// # let authenticator = SteamAuthenticator::new(user);
    ///
    /// // .. authenticator setup and login above
    ///
    /// # let mut confirmations = Confirmations::default();
    /// confirmations.filter_by_confirmation_type(EConfirmationType::Trade);
    ///
    /// authenticator
    ///     .process_confirmations(ConfirmationMethod::Accept, confirmations)
    ///     .await
    ///     .unwrap();
    /// # }
    /// ```
    pub fn filter_by_confirmation_type(
        &self,
        confirmation_type: EConfirmationType,
    ) -> impl Iterator<Item = &Confirmation> {
        self.0
            .iter()
            .filter(move |confirmation| confirmation.kind == confirmation_type)
    }

    /// Filters [`Confirmations`] based on trade offer ids.
    ///
    /// # Example
    /// ```no_run
    /// # use steam_mobile::{ConfirmationMethod, EConfirmationType, User};
    /// # use steam_mobile::SteamAuthenticator;
    /// # use steam_mobile::Confirmations;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// // .. authenticator fetch confirmations..
    /// # let mut confirmations = Confirmations::default();
    /// let trade_offer_ids = vec![40845647i64, 40844784i64]; // Could be a service or api call
    /// confirmations.filter_by_trade_offer_ids(&trade_offer_ids);
    /// # }
    /// ```
    pub fn filter_by_trade_offer_ids<'a, 'b, T>(&'a self, trade_offer_ids: T) -> impl Iterator<Item = &'a Confirmation>
    where
        T: AsRef<[i64]> + 'a,
    {
        self.0.iter().filter(move |c| {
            c.details.map_or(false, |conf_details| {
                let trade_offer_id = conf_details
                    .trade_offer_id
                    .expect("Confirmation detail comes from a trade and must have a trade_offer_id.");
                trade_offer_ids.as_ref().iter().any(|&id| id == trade_offer_id)
            })
        })
    }

    /// Checks if any of the `Confirmation` have the `trade_offer_id`.
    pub fn has_trade_offer_id(&self, trade_offer_id: i64) -> bool {
        self.0.iter().any(|conf| {
            conf.details
                .as_ref()
                .map_or(false, |details| details.trade_offer_id == Some(trade_offer_id))
        })
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

    #[test]
    fn filter_confirmation_type() {
        let confirmations = get_confirmations();
        assert_eq!(confirmations.len(), 4);

        let filtered_confirmations: Vec<_> = confirmations
            .filter_by_confirmation_type(EConfirmationType::Market)
            .collect();
        assert_eq!(filtered_confirmations.len(), 1);
    }

    #[test]
    fn has_tradeoffer_id() {
        let confirmations = get_confirmations();
        assert!(confirmations.has_trade_offer_id(4000980011));
        assert!(!confirmations.has_trade_offer_id(4000793104));
    }

    #[test]
    fn filter_trade_offer_id() {
        let confirmations = get_confirmations();
        let first = 4009687284;
        let second = 4000793103;
        let third = 33311221; // no existant
        let tradeoffer_id = vec![first, second, third];

        let details_0 = ConfirmationDetails {
            trade_offer_id: Some(first),
        };
        let details_1 = ConfirmationDetails {
            trade_offer_id: Some(second),
        };

        let filtered = confirmations
            .filter_by_trade_offer_ids(tradeoffer_id)
            .collect::<Vec<_>>();
        assert_eq!(filtered.first().unwrap().details, Some(details_0));
        assert_eq!(filtered.get(1).unwrap().details, Some(details_1));
        assert_eq!(filtered.get(2), None);
    }
}
