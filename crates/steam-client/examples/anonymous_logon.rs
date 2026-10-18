//! Connects to a Steam CM server, logs on anonymously, spawns the client, stays idle for a while, calls a service
//! method, and logs off.
//!
//! Run with `RUST_LOG=steam_client=debug cargo run -p steam-client-lib --example anonymous_logon`. The idle time is 210
//! s (Steam drops a client that sends no heartbeat within about 3 min); set `IDLE_SECONDS` to change it.

use std::time::Duration;
use std::time::Instant;

use steam_client::client::SteamClient;
use steam_client::errors::ClientError;
use steam_client::events::Event;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Request;

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    env_logger::init();
    let idle = Duration::from_secs(
        std::env::var("IDLE_SECONDS")
            .ok()
            .and_then(|seconds| seconds.parse().ok())
            .unwrap_or(210),
    );

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

    // the task sends the heartbeats while nothing calls
    let (handle, mut events, task) = client.spawn();
    println!("Spawned, idle for {:?}", idle);
    let start = Instant::now();
    let deadline = tokio::time::sleep(idle);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(Event::Message(message)) => println!("Event: {:?}", message.emsg()),
                Some(event) => println!("Event: {:?}", event),
                // Steam ended the session for good
                None => {
                    eprintln!("The client stopped: {:?}", task.await);
                    return Err(ClientError::Closed);
                }
            },
            () = &mut deadline => break,
        }
    }

    // a non-authed method, for an account that doesn't exist
    let mut request = CAuthentication_GetPasswordRSAPublicKey_Request::new();
    request.set_account_name("steamhelper-rs-nonexistent-account".to_string());
    let call = Instant::now();
    let called = handle.call_service_method(request).await;
    if let Ok(response) = &called {
        println!(
            "After {:?} idle, called Authentication.GetPasswordRSAPublicKey in {:?}: a {}-bit key",
            start.elapsed(),
            call.elapsed(),
            response.publickey_mod().len() * 4
        );
    }

    // logs off whether the call worked or not
    match handle.log_off().await {
        Ok(()) => println!("Logged off"),
        // the client stops anyway
        Err(error) => eprintln!("Couldn't log off: {}", error),
    }
    println!("Stopped: {:?}", task.await);
    called.map(drop)
}
