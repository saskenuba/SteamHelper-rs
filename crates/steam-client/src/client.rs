use std::error::Error;

use steamid_parser::SteamID;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::config::SteamConfiguration;
use crate::connection::SteamConnection;

#[derive(Debug)]
pub struct SteamClient<S>
where
    S: AsyncRead + AsyncWrite,
{
    /// Could be standard tcp or websockets (default).
    connection: SteamConnection<S>,
    /// Configuration to be used.
    configuration: SteamConfiguration,
    /// Server list.
    server_list: Vec<String>,
    /// SteamID.
    steam_id: SteamID,
    /// Your API Key if you want to do some commands
    api_key: Option<String>,
    /// CellID it is about the region you are going to fetch Steam servers
    cell_id: Option<String>,
}

impl<S> SteamClient<S>
where
    S: AsyncRead + AsyncWrite,
{
    /// Constructs a basic steam client
    pub fn builder() {}

    pub fn with_configuration(&mut self, cfg: SteamConfiguration) {}

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        unimplemented!()
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
