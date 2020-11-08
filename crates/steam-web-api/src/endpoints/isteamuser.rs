use steam_web_api_derive::{interface, Parameters};

import!();

use crate::response_types::{GetPlayerBansResponseBase, GetPlayerSummariesResponseBase};

new_type!(ISteamUser);
impl_conversions!(@GetQueryBuilder -> @ISteamUser);
convert_with_endpoint!(@GetQueryBuilder -> @ISteamUser);

#[interface(ISteamUser)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct GetPlayerSummariesParameters {
    #[comma]
    steamids: Vec<String>,
}

#[interface(ISteamUser)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct GetPlayerBansParameters {
    #[comma]
    steamids: Vec<String>,
}

#[interface(ISteamUser)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct ResolveVanityURLParameters {
    vanityurl: String,
}

#[interface(ISteamUser)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct GetFriendListParameters {
    steamid: u64,
    relationship: String,
}

convert_with_endpoint!(@ISteamUser -> GetPlayerBans |> "GetPlayerBans/v1");
convert_with_endpoint!(@ISteamUser -> GetPlayerSummaries |> "GetPlayerSummaries/v2");
convert_with_endpoint!(@ISteamUser -> ResolveVanityURL |> "ResolveVanityURL/v1");
convert_with_endpoint!(@ISteamUser -> GetFriendList |> "GetFriendList/v1");

impl_executor!(GetFriendList);
impl_executor!(ResolveVanityURL);
impl_executor!(GetPlayerSummaries -> GetPlayerSummariesResponseBase);
impl_executor!(GetPlayerBans -> GetPlayerBansResponseBase);