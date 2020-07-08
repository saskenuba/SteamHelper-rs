use crate::types::trade_offer_web::{Asset, AssetList};

#[derive(Debug, PartialEq, Clone)]
pub struct AssetCollection(pub(crate) Vec<Asset>);

impl AssetCollection {
    pub fn dump_to_asset_list(self) -> AssetList {
        AssetList {
            assets: self.0,
            ..Default::default()
        }
    }

    pub fn add(&mut self, appid: u32, contextid: u32, assetid: u64) {
        let asset = Asset {
            appid,
            contextid: contextid.to_string(),
            amount: 1,
            assetid: assetid.to_string(),
        };

        // if self.0.is_none() {
        //     self.0 = Some(Vec::new())
        // }

        self.0.push(asset);
    }
}

impl Default for AssetCollection {
    fn default() -> Self {
        Self { 0: vec![] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oi() {
        let mut my_assets = AssetCollection::default();
        my_assets.add(570, 2, 17034419698);
        my_assets.add(730, 2, 18465222145);
        println!(
            "{:?}",
            serde_json::to_string(&my_assets.dump_to_asset_list())
        );
    }
}

// 1 ask -> version 2
// 2 ask -> version 5
// 3 ask -> version 4
// 4 ask -> version 7
