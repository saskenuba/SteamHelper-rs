use std::result::Result;

use reqwest::blocking::Response;
use reqwest::Error;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct CmServerWebApi {
    response: CmServerResponse,
}

#[derive(Deserialize, Debug)]
struct CmServerResponse {
    serverlist: Vec<String>,
    serverlist_websockets: Vec<String>,
    result: u8,
    message: String,
}

/// Requests login servers from Steam WEB API
pub fn fetch_servers(api_key: &str) -> Result<CmServerWebApi, Error> {
    let url = &format!(
        "https://api.steampowered.com/ISteamDirectory/GetCMList/v1/\
         ?key={API_KEY}&cellid={STEAM_ID}",
        API_KEY = api_key,
        STEAM_ID = 0
    );

    let json: CmServerWebApi = reqwest::blocking::get(url)?.json()?;
    Ok(json)
}

/// Requests server from cm0.steampowered.com
pub fn fetch_servers_fallback(api_key: &str) -> Result<String, Error> {
    let url = "cm0.steampowered.com";

    unimplemented!()
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn fetch_servers_web_api() {
        let get_results = fetch_servers(env!("STEAM_API"));
        let servers: CmServerWebApi = get_results.unwrap();
        println!("{:?}", servers.response.serverlist);
    }

}
