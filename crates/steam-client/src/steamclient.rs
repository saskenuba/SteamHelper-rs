use std::error::Error;

use crate::{cmserver::CmServerSvList, config::SteamConfiguration, connection::SteamConnection};

use steamid_parser::SteamID;

pub struct SteamClient<S> {
    /// Could be standard tcp or websockets (default).
    connection: SteamConnection<S>,
    /// Configuration to be used.
    configuration: SteamConfiguration,
    /// Server list.
    server_list: CmServerSvList,
    /// SteamID.
    steam_id: SteamID,
    /// Your API Key if you want to do some commands
    api_key: Option<String>,
    /// CellID it is about the region you are going to fetch Steam servers
    cell_id: Option<String>,
}

impl<S> SteamClient<S> {
    /// Constructs a basic steam client
    pub fn builder() {}

    pub fn with_configuration(&mut self, cfg: SteamConfiguration) {}

    pub async fn connect(&self) -> Result<(), Box<dyn Error>> {
        CmServerSvList::fetch_servers(self.api_key.as_ref().unwrap().as_ref()).await?;
        Ok(())
    }
}

// we need to have handlers for categorized events (friends, trading, profile actions.. etc)
// and each of these handlers need to have one callback for each action
//
// The example would be the user related actions: [SteamUser] with the Handler trait
// It has a callback for the logged on event

// user defined callback
// fn on_logged_on() {
//    println!("Successfully logged in!");
// }
