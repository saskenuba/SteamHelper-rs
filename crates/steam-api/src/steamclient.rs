use crate::cmserver::CmServerSvList;
use crate::config::SteamConfiguration;
use crate::connection::SteamConnection;
use crate::steam_id::SteamID;

struct SteamClient<S> {
    /// Could be standard tcp or websockets (default).
    connection: SteamConnection<S>,
    /// Configuration to be used.
    configuration: SteamConfiguration,
    /// Server list.
    server_list: CmServerSvList,
    /// SteamID.
    steam_id: SteamID,
}

impl<S> SteamClient<S> {

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
