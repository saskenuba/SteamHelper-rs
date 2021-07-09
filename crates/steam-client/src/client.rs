use std::error::Error;
use std::sync::Arc;

use steamid_parser::SteamID;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use crate::config::SteamConfiguration;
#[cfg(feature = "websockets")]
use crate::connection::WsStream;
use crate::connection::{CMConnectionExt, SteamConnection};
use crate::content_manager::dump_tcp_servers;
use crate::errors::ConnectionError;
use crate::handlers::dispatcher::DispatcherMap;

struct SteamClient {
    /// Socket connection to CM Server.
    #[cfg(not(feature = "websockets"))]
    cm_connection: SteamConnection<TcpStream>,
    /// Socket connection to CM Server.
    #[cfg(feature = "websockets")]
    connection: SteamConnection<WsStream>,

    /// Server list.
    server_list: Vec<String>,
    /// SteamID.
    steam_id: SteamID,
    /// Steam API key. Retrieved after logon to account.
    api_key: Option<String>,
    /// CellID it is about the region you are going to fetch Steam servers
    cell_id: Option<String>,

    /// EventHandlers the user is interested in interacting.
    event_handlers: EventHandlers,
    /// Main dispatcher of messages.
    dispatcher: DispatcherMap,
}

struct EventHandlers {
    user_handler: Option<Arc<dyn UserHandler>>,
    friends_handler: Option<Arc<dyn FriendsHandler>>,
}

impl SteamClient {
    /// Constructs a basic steam client
    pub fn builder() -> ClientBuilder {
        ClientBuilder {
            user_handler: None,
            friends_handler: None,
        }
    }

    pub fn with_configuration(&mut self, cfg: SteamConfiguration) {}

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        unimplemented!()
    }

    pub async fn start(&mut self) -> Result<(), ConnectionError> {
        // initial setup for the client,
        // fetching available CM servers
        // connecting to them
        // setting up channels for message passing

        let mut dumped_cm_servers = dump_tcp_servers().await.unwrap();
        self.cm_connection = SteamConnection::new_connection(dumped_cm_servers.remove(0)).await?;
        Ok(())
    }

    // todo: should be abortable, check the `abortable` and `remote_handle`
    pub async fn start_heartbeat(time: i64, sender_channel: i64) {
        tokio::spawn(async move { async { 1 }.await });
    }
}

struct ClientBuilder {
    user_handler: Option<Box<dyn UserHandler>>,
    friends_handler: Option<Box<dyn FriendsHandler>>,
}

impl ClientBuilder {
    pub fn user_handler(handler: impl UserHandler) {}

    pub fn friends_handler(handler: impl FriendsHandler) {}

    // pub fn finish(self) -> SteamClient {}
}

trait UserHandler: Send + Sync {
    /// Called as soon as Steam is connected to Steam3.
    fn on_connected(&self, cx: ConnectedContext) {}

    fn logon(&self, cx: ConnectedContext) {}
}

/// On this state we have limited options
struct ConnectedContext {}

/// On this state we have additional options
struct LoggedOnContext {}

trait FriendsHandler: Send + Sync {
    fn basinga(&self);
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn aa() {
//         SteamClient::builder().user_handler();
//
//     }
// }
//
// pub async fn on_logged_on(cx: Context)  {
//     let response = cx.steam_user().logon(logon_details).await;
//
//     response.is_ok() {
//         println!("Logon successfull");
//     }
// }
//
//
// pub async fn on_connected(cx: Context)  {
//     let response = cx.steam_user().logon(logon_details).await;
//
//     response.is_ok() {
//         println!("Logon successfull");
//     }
// }

// we need to have handlers for categorized events (friends, trading, profile actions.. etc)
// and each of these handlers need to have one callback for each action
//
// The example would be the user related actions: [SteamUser] with the Handler trait
// It has a callback for the logged on event

// user defined callback
// fn on_logged_on() {
//    println!("Successfully logged in!");
// }
