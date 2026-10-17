//! Connects to a Steam CM server, logs on anonymously, waits for a heartbeat, and logs off.
//!
//! Run with `RUST_LOG=steam_client=debug cargo run -p steam-client --example anonymous_logon`.

use steam_client::client::SteamClient;
use steam_client::errors::ClientError;

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    env_logger::init();

    let client = SteamClient::connect().await?;
    println!("Connected to {}", client.endpoint());

    let client = client.login_anonymous().await?;
    println!("Logged on: EResult OK");
    println!(
        "SteamID: {} {}",
        client.steam_id().to_steam64(),
        client.steam_id().to_steam3_string()
    );
    println!("Session id: {}", client.session_id());
    println!("Cell id: {}", client.cell_id());
    println!("Heartbeat interval: {:?}", client.heartbeat_interval());

    // stay logged on past a heartbeat
    tokio::time::sleep(client.heartbeat_interval() * 2 + std::time::Duration::from_secs(1)).await;

    client.log_off().await?;
    println!("Logged off");
    Ok(())
}
