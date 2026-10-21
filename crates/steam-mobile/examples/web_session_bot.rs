//! The long-running setup for steam-mobile and steam-trading: logs on with a saved refresh token, spawns the client,
//! logs a steam-mobile authenticator in to the Steam websites through it (`login_with_client`), and reads events
//! until Ctrl-C. The authenticator keeps its web session fresh by itself: the first request made within an hour of the
//! access token's expiry (`renew_before` sets another margin) first gets a new one through the client.
//!
//! ```sh
//! RUST_LOG=steam_client=info,steam_mobile=info cargo run -p steam-mobile --features cm --example web_session_bot
//! ```
//!
//! Reads, from these paths or the environment variables named:
//! - `STEAM_MAFILE` (`~/.steam-test/account.maFile`): the maFile, for the account name and the authenticator.
//! - `STEAM_PASSWORD_FILE` (`~/.steam-test/password`): the password, which steam-mobile's user takes. It is never read
//!   from the maFile, and the CM logon doesn't use it.
//! - `STEAM_REFRESH_TOKEN_FILE` (`~/.steam-test/refresh_token`): the refresh token the `login` example saved.
//!
//! Prints the connection's ups and downs; `steam_mobile=info` logs each renewal of the web session. Prints no secret.
//!
//! # Rotating the refresh token
//!
//! A refresh token lasts about 200 days; a bot that runs longer lets Steam replace it with `renew_tokens`, and saves
//! the new one, as the old one is revoked. With `ROTATE_TOKENS=1`, this does it once a day (`ROTATE_HOURS` to
//! change it) and saves the new token to `STEAM_REFRESH_TOKEN_FILE` (mode 0600). **Off by default: step L8 of the
//! live test plan, not verified live.** The client keeps the new token, and logs on again with it, as soon as Steam's
//! answer comes, even if the call failed or timed out on our side meanwhile: so the token to save is always
//! `handle.refresh_token()`, read after each attempt, whatever its result.
//!
//! The authenticator is what steam-mobile (confirmations) and steam-trading (`SteamTradeManager::new(authenticator)`,
//! which owns it) take; the bot's own work goes where the comment in the loop says.
//!
//! Not tried live: it runs steps L1, L3 and L7 of the live test plan (`crates/steam-client/workingcontext.md`), through
//! a spawned client; L7 only once the bot makes requests, which renew the token when it is due.

use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;

use steam_client::auth::RefreshToken;
use steam_client::client::SteamClient;
use steam_client::client::User;
use steam_client::events::Event;
use steam_client::handle::SteamHandle;
use steam_mobile::user::SteamUser;
use steam_mobile::MobileAuthFile;
use steam_mobile::SteamAuthenticator;
use tokio::time::Instant;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// How long to wait before trying to rotate the refresh token again after a failure, such as while the client
/// reconnects.
const RETRY_AFTER: Duration = Duration::from_secs(30);

/// Saves the refresh token, readable by the owner only.
fn save(path: &Path, token: &RefreshToken) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(token.expose_secret().as_bytes())?;
    Ok(())
}

/// Lets Steam replace the refresh token, and saves the client's latest one if it changed: Steam revoked the old one.
async fn rotate(handle: &SteamHandle<User>, path: &Path, saved: &mut RefreshToken) -> Result<()> {
    let renewed = handle.renew_tokens().await;
    // read whatever the result: a token Steam replaced is kept by the client even if the call failed on our side
    let latest = handle.refresh_token();
    if latest.expose_secret() != saved.expose_secret() {
        save(path, &latest)?;
        println!(
            "Steam replaced the refresh token; saved, it expires {}",
            until(latest.expires_at())
        );
        *saved = latest;
    }
    renewed?;
    Ok(())
}

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

/// How long until `time`, for printing expiries.
fn until(time: SystemTime) -> String {
    match time.duration_since(SystemTime::now()) {
        Ok(left) => format!("in {}h{:02}m", left.as_secs() / 3600, left.as_secs() / 60 % 60),
        Err(_) => "already".to_string(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let mafile = MobileAuthFile::from_disk(path("STEAM_MAFILE", "account.maFile")?)?;
    let account = mafile.account_name.clone().ok_or("the maFile has no account_name")?;
    let password = fs::read_to_string(path("STEAM_PASSWORD_FILE", "password")?)?;
    let password = password.trim_end_matches(['\r', '\n']).to_string();
    let token_path = path("STEAM_REFRESH_TOKEN_FILE", "refresh_token")?;
    let token = fs::read_to_string(&token_path).map_err(|_| "no saved refresh token: run the login example first")?;
    let token = RefreshToken::new(token.trim().to_string())?;
    // step L8, unverified live: see the module docs
    let rotate_every = match std::env::var("ROTATE_TOKENS").as_deref() {
        Ok("1") => {
            let hours = std::env::var("ROTATE_HOURS")
                .ok()
                .and_then(|hours| hours.parse().ok())
                .unwrap_or(24);
            Some(Duration::from_secs(hours * 3600))
        }
        _ => None,
    };
    let mut saved = token.clone();

    let client = SteamClient::connect()
        .await?
        .login_with_refresh_token(&account, token)
        .await?;
    println!("Logged on as {}", client.steam_id().to_steam64());
    let (handle, mut events, task) = client.spawn();

    let user = SteamUser::new(account, password).with_mafile(mafile);
    let authenticator = SteamAuthenticator::new(user).login_with_client(handle.clone()).await?;
    println!(
        "Logged in to the Steam websites as {}",
        authenticator.steam_id().to_steam64()
    );
    // never fires without ROTATE_TOKENS
    let far = Duration::from_secs(100 * 365 * 86400);
    let rotation = tokio::time::sleep(rotate_every.unwrap_or(far));
    tokio::pin!(rotation);

    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            event = events.recv() => match event {
                // renewing the web session needs the connection: while it is down, requests go on with the current
                // token until it expires, then fail unsent, and a renewal is tried again 30 seconds after one failed
                // at the soonest
                Some(Event::Disconnected { reason }) => println!("Disconnected ({:?}), connecting again", reason),
                Some(Event::Reconnected) => println!("Logged on again"),
                Some(Event::Lagged(missed)) => eprintln!("Missed {} events", missed),
                // for good, such as a revoked refresh token (`AccessDenied`): the last event, the task stopped
                Some(Event::LoggedOff(result)) => println!("Steam logged the client off: {:?}", result),
                // the bot's work: Steam's pushes, such as trade offer notifications, then steam-trading with a
                // `SteamTradeManager` made from `authenticator`
                Some(_) => {}
                None => break,
            },
            () = &mut rotation => {
                if let Err(error) = rotate(&handle, &token_path, &mut saved).await {
                    eprintln!("Couldn't rotate the refresh token, trying again in {:?}: {}", RETRY_AFTER, error);
                    rotation.as_mut().reset(Instant::now() + RETRY_AFTER);
                    continue;
                }
                rotation.as_mut().reset(Instant::now() + rotate_every.unwrap_or(far));
            }
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
