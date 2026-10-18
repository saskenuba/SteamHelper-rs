//! What does Steam send me? Logs on anonymously, spawns the client, prints every event for a while, and logs off.
//!
//! Run with `cargo run -p steam-client --example events`. It listens for 30 s; set `EVENT_SECONDS` to change it, and
//! `RUST_LOG=steam_client=debug` to see the connection's own log.
//!
//! Each message prints its EMsg, and a service notification (`EMsg::ServiceMethod`) also the method Steam called, as
//! `Service.Method#1`. To read a message, parse it as its steam-protobuf type with `message.parse_proto::<T>()`;
//! for a notification, `event.notification::<N>()` checks the method too, `N` naming it: one of
//! `steam_client::service::notifications`, or a marker type of yours (see `ServiceNotification`).
//!
//! An anonymous logon gets a single message, `ClientServersAvailable`, right after logon; an account gets many more.
//! Expect `Logged on to <server> as <SteamID>`, a line per event with the time it came, such as
//! `24.9µs  ClientServersAvailable`, then a count per kind, `Logged off`, and `Stopped: LoggedOff`.

use std::collections::BTreeMap;
use std::time::Duration;
use std::time::Instant;

use steam_client::client::SteamClient;
use steam_client::errors::ClientError;
use steam_client::events::Event;

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    env_logger::init();
    let listen = Duration::from_secs(
        std::env::var("EVENT_SECONDS")
            .ok()
            .and_then(|seconds| seconds.parse().ok())
            .unwrap_or(30),
    );

    let client = SteamClient::connect().await?.login_anonymous().await?;
    println!("Logged on to {} as {}", client.endpoint(), client.steam_id());
    let (handle, mut events, task) = client.spawn();

    println!("Listening for {:?}", listen);
    let start = Instant::now();
    let mut seen = BTreeMap::new();
    let deadline = tokio::time::sleep(listen);
    tokio::pin!(deadline);
    loop {
        let event = tokio::select! {
            event = events.recv() => event,
            () = &mut deadline => break,
        };
        let elapsed = start.elapsed();
        let name = match event {
            Some(Event::Message(message)) => match message.service_method() {
                Some(method) => format!("ServiceMethod {}", method),
                None => format!("{:?}", message.emsg()),
            },
            // the connection's own events: see examples/reconnect.rs
            Some(event) => format!("{:?}", event),
            // Steam ended the session for good: the task stopped
            None => break,
        };
        println!("{:>6.1?}  {}", elapsed, name);
        *seen.entry(name).or_insert(0) += 1;
    }

    println!("In {:?}:", start.elapsed().min(listen));
    for (name, count) in &seen {
        println!("  {:>4} × {}", count, name);
    }
    match handle.log_off().await {
        Ok(()) => println!("Logged off"),
        // also when the client stopped already
        Err(error) => eprintln!("Couldn't log off: {}", error),
    }
    // the connection is closed once the task ends
    println!("Stopped: {:?}", task.await);
    Ok(())
}
