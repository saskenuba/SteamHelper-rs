//! Steam's ids of offers, trades and items, typed so that one can't be passed for another.

use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

use serde::Deserialize;
use serde::Serialize;

use crate::InventoryContext;

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        ///
        /// Shown and parsed as the bare number; in JSON (serde) the number itself.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = ParseIntError;

            fn from_str(id: &str) -> Result<Self, Self::Err> {
                id.parse().map(Self)
            }
        }
    };
}

id!(
    /// A trade offer's id (`tradeofferid`).
    OfferId
);
id!(
    /// A trade's id (`tradeid`): what an accepted offer became.
    TradeId
);
id!(
    /// An item's id in its owner's inventory (`assetid`). A trade gives the item a new one.
    AssetId
);

/// An item, where it is: its inventory context and its id there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Asset {
    pub context: InventoryContext,
    pub id: AssetId,
}

impl Asset {
    pub const fn new(context: InventoryContext, id: AssetId) -> Self {
        Self { context, id }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_numbers() {
        let id: OfferId = "9438483375".parse().unwrap();
        assert_eq!(id, OfferId(9438483375));
        assert_eq!(id.to_string(), "9438483375");
        assert_eq!(
            serde_json::to_string(&TradeId(695401084301346863)).unwrap(),
            "695401084301346863"
        );
        assert_eq!(
            serde_json::from_str::<AssetId>("24234602605").unwrap(),
            AssetId(24234602605)
        );
        assert!("-1".parse::<AssetId>().is_err());
    }
}
