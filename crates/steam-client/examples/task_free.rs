//! Drives a client without spawning it: one loop waits for Steam's events and, on a timer, calls Steam.
//!
//! Run with `cargo run -p steam-client-lib --example task_free`. It runs 210 s, calling every 60 s; set `RUN_SECONDS`
//! and `CALL_SECONDS` to change them, and `RUST_LOG=steam_client=debug` to see the heartbeats go out. Expect
//! `Logged on`, the few events of an anonymous logon, a call answered every minute, then `Logged off`.
//!
//! An unspawned client has no task: it reads and writes the connection only while one of its methods is awaited.
//! So it must always be: whenever it isn't calling, await [`next_event`], which reads what Steam sends and sends the
//! heartbeats. A client left idle sends none, and Steam drops it within a few minutes (alive after 90 s, gone at
//! 180 s, measured). Nor does it connect again: once the connection is gone, `next_event` fails with `Dropped`.
//! `spawn` does both for you (see examples/reconnect.rs); this suits a short-lived tool, or a program that already
//! has its own loop.
//!
//! [`next_event`]: steam_client::client::SteamClient::next_event

use std::time::Duration;
use std::time::Instant;

use steam_client::client::SteamClient;
use steam_client::errors::ClientError;
use steam_client::events::Event;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Request;

fn seconds(variable: &str, default: u64) -> Duration {
    Duration::from_secs(
        std::env::var(variable)
            .ok()
            .and_then(|seconds| seconds.parse().ok())
            .unwrap_or(default),
    )
}

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    env_logger::init();
    let run = seconds("RUN_SECONDS", 210);
    let call_every = seconds("CALL_SECONDS", 60);

    let mut client = SteamClient::connect().await?.login_anonymous().await?;
    println!("Logged on to {} as {}", client.endpoint(), client.steam_id());

    let start = Instant::now();
    let mut calls = tokio::time::interval_at(tokio::time::Instant::now() + call_every, call_every);
    let deadline = tokio::time::sleep(run);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            // cancel-safe: when the timer fires first, nothing Steam sent is lost
            event = client.next_event() => match event? {
                Event::Message(message) => println!("{:>6.1?}  {:?}", start.elapsed(), message.emsg()),
                event => println!("{:>6.1?}  {:?}", start.elapsed(), event),
            },
            _ = calls.tick() => {
                let mut request = CAuthentication_GetPasswordRSAPublicKey_Request::new();
                request.set_account_name("steamhelper-rs-nonexistent-account".to_string());
                // awaiting the call drives the connection too
                client.call_service_method(request).await?;
                println!("{:>6.1?}  Called Steam: answered", start.elapsed());
            }
            () = &mut deadline => break,
        }
    }

    client.log_off().await?;
    println!("Logged off after {:?}", start.elapsed());
    Ok(())
}
