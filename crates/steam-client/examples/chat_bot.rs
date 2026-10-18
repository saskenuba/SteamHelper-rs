//! An echo bot: logs on with a saved refresh token, shows itself online, and answers each friend's message with
//! "You said: ...", until Ctrl-C.
//!
//! ```sh
//! STEAM_ACCOUNT=<account name> RUST_LOG=steam_client=info cargo run -p steam-client --example chat_bot
//! ```
//!
//! Reads:
//! - `STEAM_ACCOUNT`: the account name.
//! - `STEAM_REFRESH_TOKEN_FILE` (`~/.steam-test/refresh_token`): the refresh token the `login` example saved.
//!
//! Prints each message and who sent it, and when the connection is lost and back (the client logs on again by
//! itself, and shows itself online again). Prints no token.
//!
//! **Not verified live.** Account logons ask for the new Steam chat (`chat_mode = 2`) and the bot sets its persona
//! online, as node-steam-user does for friend messages; whether Steam then sends them to this crate is untested.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use steam_client::auth::RefreshToken;
use steam_client::client::SteamClient;
use steam_client::client::User;
use steam_client::events::Event;
use steam_client::handle::SteamHandle;
use steam_client::service::notifications::IncomingMessage;
use steam_language_gen::generated::enums::EChatEntryType;
use steam_language_gen::generated::enums::EPersonaState;
use steam_protobuf::protobufs::steammessages_friendmessages_steamclient::CFriendMessages_IncomingMessage_Notification;
use steam_protobuf::protobufs::steammessages_friendmessages_steamclient::CFriendMessages_SendMessage_Request;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `variable`, or `~/.steam-test/<file>`.
fn path(variable: &str, file: &str) -> Result<PathBuf> {
    match std::env::var_os(variable) {
        Some(path) => Ok(path.into()),
        None => {
            let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
            Ok(Path::new(&home).join(".steam-test").join(file))
        }
    }
}

/// Answers a friend's message, in a task of its own, so the events keep being read meanwhile.
fn echo(handle: &SteamHandle<User>, incoming: CFriendMessages_IncomingMessage_Notification) {
    // our own messages, sent from another session (and echoed to this one), and typing notifications
    if incoming.local_echo() || incoming.chat_entry_type() != EChatEntryType::ChatMsg as i32 {
        return;
    }
    let friend = incoming.steamid_friend();
    println!("{} says: {}", friend, incoming.message());

    let mut reply = CFriendMessages_SendMessage_Request::new();
    reply.set_steamid(friend);
    reply.set_chat_entry_type(EChatEntryType::ChatMsg as i32);
    reply.set_message(format!("You said: {}", incoming.message()));
    let handle = handle.clone();
    tokio::spawn(async move {
        if let Err(error) = handle.call_service_method(reply).await {
            eprintln!("Couldn't answer {}: {}", friend, error);
        }
    });
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let account = std::env::var("STEAM_ACCOUNT").map_err(|_| "set STEAM_ACCOUNT to the account name")?;
    let token_path = path("STEAM_REFRESH_TOKEN_FILE", "refresh_token")?;
    let token = fs::read_to_string(&token_path).map_err(|_| "no saved refresh token: run the login example first")?;
    let token = RefreshToken::new(token.trim().to_string())?;

    let client = SteamClient::connect()
        .await?
        .login_with_refresh_token(&account, token)
        .await?;
    println!("Logged on as {}", client.steam_id().to_steam64());
    let (handle, mut events, task) = client.spawn();
    // offline until set otherwise; the client sets it again each time it logs on again
    handle.set_persona_state(EPersonaState::Online)?;
    println!("Online, answering messages until Ctrl-C");

    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            event = events.recv() => match event {
                // the client logs on again by itself, such as when Steam asks it to move (`TryAnotherCM`)
                Some(Event::Disconnected { reason }) => println!("Disconnected ({:?}), connecting again", reason),
                Some(Event::Reconnected) => println!("Logged on again"),
                Some(Event::Lagged(missed)) => eprintln!("Missed {} events", missed),
                // for good, such as the account logging on elsewhere: the last event, the task stopped
                Some(Event::LoggedOff(result)) => println!("Steam logged the bot off: {:?}", result),
                Some(event) => match event.notification::<IncomingMessage>() {
                    Some(Ok(incoming)) => echo(&handle, incoming),
                    Some(Err(error)) => eprintln!("Unreadable friend message: {}", error),
                    None => {}
                },
                None => break,
            },
            result = &mut ctrl_c => {
                result?;
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
                break;
            }
        }
    }
    println!("Stopped: {:?}", task.await);
    Ok(())
}
