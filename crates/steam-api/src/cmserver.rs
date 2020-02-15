use std::result::Result;

use reqwest::Error;
use serde::Deserialize;

use crate::webapi::APIBuilder;

#[derive(Deserialize, Debug)]
pub struct CmServerSvList {
    response: CmServerSvListResponse,
}

#[derive(Deserialize, Debug)]
pub struct CmServerSvListResponse {
    serverlist: Vec<String>,
    serverlist_websockets: Vec<String>,
    result: u8,
    message: String,
}

impl CmServerSvList {
    pub fn dump_tcp_servers(&self) -> Vec<String> {
        self.response.serverlist.clone()
    }
    pub fn dump_ws_servers(&self) -> Vec<String> {
        self.response.serverlist_websockets.clone()
    }

    /// Requests login servers from Steam WEB API
    /// Steam calls regions as Cells
    /// reference: https://github.com/SteamDatabase/SteamTracking/blob/master/ClientExtracted/steam/cached/CellMap.vdf
    pub async fn fetch_servers(api_key: &str) -> Result<CmServerSvList, Error> {
        let parameters = vec![("cellid", "0")];

        let new_api_call =
            APIBuilder::new(
                "ISteamDirectory",
                "GetCMList",
                api_key,
                Option::from(parameters),
            );

        let json: CmServerSvList = new_api_call.setup().await?.json().await?;
        debug!("Fetched servers successfully: {:?}", json);
        trace!("Complete response {:?}", json.response);

        Ok(json)
    }

    ///
    pub async fn fetch_servers_fallback(api_key: &str) -> Result<String, Error> {
        let url = "cm0.steampowered.com";
        unimplemented!()
    }
}


#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;


    #[tokio::test]
    async fn fetch_servers_web_api() {
        let get_results = CmServerSvList::fetch_servers(env!("STEAM_API")).await;
        let servers: CmServerSvList = get_results.unwrap();
//        println!("Fetching servers... {:?}", servers.response.serverlist);
    }
}
