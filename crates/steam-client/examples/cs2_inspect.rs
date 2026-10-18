//! Inspects CS2 items: logs on with a saved refresh token, plays CS2, greets its game coordinator, and prints the
//! pattern, float, skin and stickers of each inspect link, until the links run out or Ctrl-C.
//!
//! ```sh
//! STEAM_ACCOUNT=<account name> RUST_LOG=steam_client=info \
//!     cargo run -p steam-client-lib --features cs2 --example cs2_inspect -- '<inspect link>' ...
//! ```
//!
//! The links are the arguments (quote them: they hold `%20` or a space), or, without any, the lines of stdin.
//!
//! Reads:
//! - `STEAM_ACCOUNT`: the account name. It must own CS2 (the free license).
//! - `STEAM_REFRESH_TOKEN_FILE` (`~/.steam-test/refresh_token`): the refresh token the `login` example saved.
//!
//! Prints no token. Links that embed their item are read offline; the others are asked to the game coordinator,
//! one at a time, about a second apart.
//!
//! **Not verified live.**

use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use steam_client::auth::RefreshToken;
use steam_client::client::SteamClient;
use steam_client::errors::ClientError;
use steam_client::events::Event;
use steam_client::gc::cs2::Cs2;
use steam_client::gc::cs2::ItemDetails;
use steam_client::handle::Events;
use tokio::sync::mpsc;

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

/// The links: the arguments, or the lines of stdin as they come, read on a thread of their own.
fn links() -> mpsc::UnboundedReceiver<String> {
    let (sender, links) = mpsc::unbounded_channel();
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.is_empty() {
        std::thread::spawn(move || {
            for line in std::io::stdin().lines() {
                let Ok(line) = line else { break };
                if !line.trim().is_empty() && sender.send(line.trim().to_string()).is_err() {
                    break;
                }
            }
        });
    } else {
        for link in arguments {
            let _ = sender.send(link);
        }
    }
    links
}

fn print(details: &ItemDetails) {
    println!(
        "  def index {}, paint index {:?}, pattern {:?}, float {:?}",
        details.def_index(),
        details.paint_index(),
        details.pattern(),
        details.float()
    );
    if let Some(stat_trak) = details.stat_trak() {
        println!("  StatTrak: {}", stat_trak.value);
    }
    if let Some(name) = details.custom_name() {
        println!("  name tag: {:?}", name);
    }
    for sticker in details.stickers() {
        println!(
            "  sticker {} in slot {}, wear {:?}",
            sticker.id, sticker.slot, sticker.wear
        );
    }
    for keychain in details.keychains() {
        println!("  keychain {}, pattern {:?}", keychain.id, keychain.pattern);
    }
}

/// Prints the connection's ups and downs: the client logs on again by itself, and greets the game coordinator again
/// at the next inspect.
async fn print_events(mut events: Events) {
    while let Some(event) = events.recv().await {
        match event {
            Event::Disconnected { reason } => eprintln!("Disconnected ({:?}), connecting again", reason),
            Event::Reconnected => eprintln!("Logged on again"),
            Event::LoggedOff(result) => eprintln!("Steam logged the account off: {:?}", result),
            _ => {}
        }
    }
}

/// Inspects each link as it comes.
async fn inspect_all(cs2: &mut Cs2) -> std::result::Result<(), ClientError> {
    let mut links = links();
    while let Some(link) = links.recv().await {
        println!("{}", link);
        match cs2.inspect_str(&link).await {
            Ok(details) => print(&details),
            Err(ClientError::Closed) => return Err(ClientError::Closed),
            Err(error) => eprintln!("  couldn't inspect it: {}", error),
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
    let (handle, events, task) = client.spawn();
    tokio::spawn(print_events(events));

    let work = async {
        let mut cs2 = Cs2::connect(&handle).await?;
        println!("CS2's game coordinator welcomed the account");
        inspect_all(&mut cs2).await
    };
    tokio::select! {
        result = work => if let Err(error) = result {
            eprintln!("Stopped: {}", error);
        },
        result = tokio::signal::ctrl_c() => result?,
    }

    // a second Ctrl-C doesn't wait for Steam
    tokio::select! {
        logged_off = handle.log_off() => match logged_off {
            // the client stopped already
            Ok(()) | Err(ClientError::Closed) => {}
            Err(error) => return Err(error.into()),
        },
        _ = tokio::signal::ctrl_c() => {
            eprintln!("Exiting without logging off");
            std::process::exit(130);
        }
    }
    println!("Logged off");
    println!("Stopped: {:?}", task.await);
    Ok(())
}
