//! A bot's lifecycle: spawn the client, follow what happens to its connection, keep calling, and stop on Ctrl-C.
//!
//! Run with `cargo run -p steam-client --example reconnect`, and stop it with Ctrl-C (a second Ctrl-C exits at once,
//! without logging off). Set `STOP=drop` to stop by dropping the handle instead of calling `log_off`, `CALL_SECONDS`
//! to change how often it calls Steam (60 s), and `RUST_LOG=steam_client=info` to see the task reconnect.
//!
//! Expect `Logged on`, `Called Steam: answered` every minute, and on Ctrl-C `Logged off` (or `Dropped the handle`),
//! then `Stopped: ...`. A live run can't show a reconnect on demand: Steam drops connections when it restarts a
//! server, and the network when it fails, at times nobody picks. To see one, cut the network (turn Wi-Fi off) for a
//! minute: calls time out, three in a row take the connection for lost (`Disconnected: Unresponsive`), or the socket
//! fails first (`ConnectionLost`); the calls meanwhile fail with `Dropped`, then `Reconnected`, to another server.
//! Steam asking the client to move (`TryAnotherCM`) is a `Disconnected` too. `LoggedOff` is a session Steam won't
//! resume, which an anonymous one rarely is: the task then stops.

use std::time::Duration;

use steam_client::client::SteamClient;
use steam_client::errors::ClientError;
use steam_client::events::Event;
use steam_client::handle::SteamHandle;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Request;

/// Calls Steam in a task of its own, so that the events keep being read while it waits.
fn call<A: Send + Sync + 'static>(handle: &SteamHandle<A>) {
    let handle = handle.clone();
    tokio::spawn(async move {
        let mut request = CAuthentication_GetPasswordRSAPublicKey_Request::new();
        request.set_account_name("steamhelper-rs-nonexistent-account".to_string());
        match handle.call_service_method(request).await {
            Ok(_) => println!("Called Steam: answered"),
            // reconnecting: the next call goes to the new connection
            Err(ClientError::Dropped) => println!("Called Steam: dropped, reconnecting"),
            // the client stopped: the events say why
            Err(ClientError::Closed) => {}
            Err(error) => println!("Called Steam: {}", error),
        }
    });
}

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    env_logger::init();
    let drop_to_stop = std::env::var("STOP").as_deref() == Ok("drop");
    let call_every = Duration::from_secs(
        std::env::var("CALL_SECONDS")
            .ok()
            .and_then(|seconds| seconds.parse().ok())
            .unwrap_or(60),
    );

    let client = SteamClient::connect().await?.login_anonymous().await?;
    println!("Logged on to {} as {}", client.endpoint(), client.steam_id());
    let (handle, mut events, task) = client.spawn();

    let mut calls = tokio::time::interval_at(tokio::time::Instant::now() + call_every, call_every);
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            event = events.recv() => match event {
                // the task connects again by itself; nothing to do but wait
                Some(Event::Disconnected { reason }) => {
                    println!("Disconnected: {:?}; calls fail with Dropped until Reconnected", reason)
                }
                // a new session, on another server: what depends on the session starts again here
                Some(Event::Reconnected) => {
                    println!("Reconnected to {} as session {}", handle.endpoint(), handle.session_id())
                }
                Some(Event::Lagged(missed)) => println!("Missed {} events: read them faster", missed),
                // for good: the last event, and the task stopped
                Some(Event::LoggedOff(result)) => println!("Steam ended the session for good: {:?}", result),
                Some(Event::Message(_)) => {}
                Some(event) => println!("{:?}", event),
                None => break,
            },
            _ = calls.tick() => call(&handle),
            _ = &mut ctrl_c => {
                if drop_to_stop {
                    // once every handle is gone, the task logs off (waiting at most 5 s for Steam) and stops; the
                    // events then end, which ends this loop
                    drop(handle);
                    println!("Dropped the handle");
                    // calls still running keep their handle until they end, at most 30 s; a second Ctrl-C doesn't wait
                    let ctrl_c = tokio::signal::ctrl_c();
                    tokio::pin!(ctrl_c);
                    loop {
                        tokio::select! {
                            event = events.recv() => match event {
                                Some(event) => println!("{:?}", event),
                                None => break,
                            },
                            _ = &mut ctrl_c => {
                                eprintln!("Exiting without logging off");
                                std::process::exit(130);
                            }
                        }
                    }
                } else {
                    // a second Ctrl-C doesn't wait for Steam
                    tokio::select! {
                        logged_off = handle.log_off() => match logged_off {
                            Ok(()) => println!("Logged off"),
                            // the client stops anyway
                            Err(error) => eprintln!("Couldn't log off: {}", error),
                        },
                        _ = tokio::signal::ctrl_c() => {
                            eprintln!("Exiting without logging off");
                            std::process::exit(130);
                        }
                    }
                }
                break;
            }
        }
    }
    // the client logged off and closed the connection, or Steam ended it
    println!("Stopped: {:?}", task.await);
    Ok(())
}
