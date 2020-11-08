use steam_web_api_derive::{interface, Parameters};

import!();

new_type!(ISteamEconomy);
impl_conversions!(@GetQueryBuilder -> @ISteamEconomy);
convert_with_endpoint!(@GetQueryBuilder -> @ISteamEconomy);

#[interface(ISteamEconomy)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub(crate) struct GetAssetClassInfoParameters {
    appid: u32,
    class_count: u32,
    #[indexed]
    classid: Vec<u32>,
    #[indexed]
    instanceid: Option<Vec<u64>>,
    language: Option<String>,
}

convert_with_endpoint!(@ISteamEconomy -> GetAssetClassInfo |> "GetClassInfo/v1");
impl_executor!(GetAssetClassInfo);
