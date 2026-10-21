//! Logs on with a saved refresh token and prints when the account's trade offers change, as Steam pushes it, until
//! Ctrl-C.
//!
//! ```sh
//! STEAM_ACCOUNT=<account name> RUST_LOG=steam_client=info cargo run -p steam-client-lib --example trade_notifications
//! ```
//!
//! Reads:
//! - `STEAM_ACCOUNT`: the account name.
//! - `STEAM_REFRESH_TOKEN_FILE` (`~/.steam-test/refresh_token`): the refresh token the `login` example saved.
//!
//! Steam may say it two ways, and it is unknown which it sends today, so this accepts both:
//! - `ClientUserNotifications` (EMsg 5599): `{ user_notification_type, count }`, type 1 being trade offers and `count`
//!   the received offers pending. Believed to come after logon and on every change.
//! - `SteamNotificationClient.NotificationsReceived#1`: notifications of type `k_ESteamNotificationType_TradeOffer`
//!   (9), their details as JSON in `body_data`.
//!
//! Either is only a trigger: it carries no offer, and likely nothing about offers we sent. On each, fetch the offers
//! over HTTP with steam-trading (`SteamTradeManager::get_trade_offers`, with the authenticator that steam-mobile's
//! `SteamAuthenticator::login_with_client` gives, as its `web_session_bot` example sets up), and poll slowly as well.
//! Notifications sent while the connection was lost are lost too: fetch once after each `Reconnected`. The design:
//! "Event-driven offers and access tokens" in `crates/steam-trading/workingcontext.md`.
//!
//! **Not verified live**: which of the two Steam sends, and when.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use steam_client::auth::RefreshToken;
use steam_client::client::SteamClient;
use steam_client::events::Event;
use steam_client::service::notifications::NotificationsReceived;
use steam_language_gen::generated::enums::EMsg;
use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgClientUserNotifications;
use steam_protobuf::protobufs::steammessages_notifications_steamclient::ESteamNotificationType;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `user_notification_type` of trade offers in `ClientUserNotifications`.
const USER_NOTIFICATION_TRADE_OFFERS: u32 = 1;

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

/// Prints what `event` says about trade offers, if anything.
fn print_trade_offers(event: &Event) -> Result<()> {
    if let Event::Message(message) = event {
        if message.emsg() == EMsg::ClientUserNotifications {
            let notifications = message.parse_proto::<CMsgClientUserNotifications>()?;
            for notification in notifications.notifications() {
                if notification.user_notification_type() == USER_NOTIFICATION_TRADE_OFFERS {
                    println!("ClientUserNotifications: {} trade offers pending", notification.count());
                }
            }
        }
    }

    if let Some(received) = event.notification::<NotificationsReceived>() {
        for notification in received?.notifications() {
            if notification.notification_type() == ESteamNotificationType::k_ESteamNotificationType_TradeOffer {
                println!(
                    "NotificationsReceived: trade offer notification {} (read: {}): {}",
                    notification.notification_id(),
                    notification.read(),
                    notification.body_data()
                );
            }
        }
    }
    Ok(())
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
    println!("Waiting for trade offer notifications until Ctrl-C");

    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(Event::Disconnected { reason }) => println!("Disconnected ({:?}), connecting again", reason),
                // a bot fetches the offers now: what changed meanwhile wasn't notified
                Some(Event::Reconnected) => println!("Logged on again"),
                Some(Event::Lagged(missed)) => eprintln!("Missed {} events", missed),
                // for good: the last event, the task stopped
                Some(Event::LoggedOff(result)) => println!("Steam logged the client off: {:?}", result),
                Some(event) => {
                    if let Err(error) = print_trade_offers(&event) {
                        eprintln!("Unreadable notification: {}", error);
                    }
                }
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
