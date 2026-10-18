//! CS2 profiles, matches and the inventory: logs on with a saved refresh token, plays CS2, greets its game
//! coordinator, prints the account's matchmaking stats and inventory, then each argument's profile and recent matches
//! (a SteamID) or match (a share code), then what the game coordinator says until Ctrl-C.
//!
//! ```sh
//! STEAM_ACCOUNT=<account name> RUST_LOG=steam_client=info \
//!     cargo run -p steam-client-lib --features cs2 --example cs2_profile -- 76561198084749846 CSGO-xxxxx-xxxxx-xxxxx-xxxxx-xxxxx
//! ```
//!
//! Reads:
//! - `STEAM_ACCOUNT`: the account name. It must own CS2 (the free license).
//! - `STEAM_REFRESH_TOKEN_FILE` (`~/.steam-test/refresh_token`): the refresh token the `login` example saved.
//!
//! Prints no token. The game coordinator answers profiles of the account's friends who play CS2 only. Requests go one
//! at a time, about a second apart.
//!
//! **Not verified live.**

use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use steam_client::auth::RefreshToken;
use steam_client::client::SteamClient;
use steam_client::errors::ClientError;
use steam_client::gc::cs2::Cs2;
use steam_client::gc::cs2::Cs2Event;
use steam_client::gc::cs2::MatchList;
use steam_client::gc::cs2::PlayerProfile;
use steam_client::gc::cs2::ShareCode;
use steamid_parser::SteamID;

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

fn print_profile(profile: &PlayerProfile) {
    println!("  level {:?}, xp {:?}", profile.level(), profile.xp());
    for rank in profile.ranks() {
        println!("  rank {} ({:?}), {} wins", rank.rank, rank.rank_type, rank.wins);
    }
    if let Some(commendations) = profile.commendations() {
        println!(
            "  commendations: {} friendly, {} teaching, {} leader",
            commendations.friendly, commendations.teaching, commendations.leader
        );
    }
    if let Some(penalty) = profile.penalty() {
        println!("  cooldown: {:?} left (reason {})", penalty.remaining, penalty.reason);
    }
    if profile.vac_banned() {
        println!("  VAC banned");
    }
}

fn print_matches(list: &MatchList) {
    if list.matches().len() == 0 {
        println!("  no matches");
    }
    for info in list.matches() {
        println!(
            "  match {:?} on {:?}: {:?}, {:?}, share code {}, demo {:?}",
            info.match_id(),
            info.map(),
            info.score(),
            info.duration(),
            info.share_code().map_or_else(String::new, |code| code.to_string()),
            info.demo_url()
        );
    }
}

/// Asks about each argument: a share code's match, or a SteamID's profile and recent matches.
async fn ask(cs2: &mut Cs2) -> std::result::Result<(), ClientError> {
    for argument in std::env::args().skip(1) {
        println!("{}", argument);
        let result = if let Ok(code) = argument.parse::<ShareCode>() {
            cs2.match_info(&code).await.map(|list| print_matches(&list))
        } else if let Ok(steam64) = argument.parse::<u64>() {
            let player = SteamID::from_steam64(steam64);
            match cs2.player_profile(player).await {
                Ok(profile) => print_profile(&profile),
                Err(ClientError::Closed) => return Err(ClientError::Closed),
                Err(error) => eprintln!("  no profile: {}", error),
            }
            cs2.recent_matches(player).await.map(|list| print_matches(&list))
        } else {
            eprintln!("  neither a share code nor a SteamID");
            continue;
        };
        match result {
            Ok(()) => {}
            Err(ClientError::Closed) => return Err(ClientError::Closed),
            Err(error) => eprintln!("  couldn't ask: {}", error),
        }
    }
    Ok(())
}

/// Prints what the game coordinator says, until the client stops.
async fn print_events(cs2: &mut Cs2) -> std::result::Result<(), ClientError> {
    loop {
        match cs2.next_event().await {
            Ok(Cs2Event::SessionLost) => eprintln!("Lost the session with the game coordinator, greeting it again"),
            Ok(Cs2Event::Welcomed) => eprintln!("Welcomed again: {} items", cs2.inventory().len()),
            Ok(Cs2Event::MatchmakingStats(stats)) => {
                println!("Matchmaking stats:");
                print_profile(&stats);
            }
            Ok(Cs2Event::InventoryLoaded) => println!("Inventory loaded again: {} items", cs2.inventory().len()),
            Ok(Cs2Event::ItemAcquired(item)) => println!("Acquired {} (def index {})", item.id(), item.def_index()),
            Ok(Cs2Event::ItemChanged { new, .. }) => println!("Changed {}", new.id()),
            Ok(Cs2Event::ItemRemoved(item)) => println!("Removed {}", item.id()),
            Ok(Cs2Event::Other(message)) => println!("Message {} ({} bytes)", message.msg_type(), message.body().len()),
            Ok(Cs2Event::Lagged(missed)) => eprintln!("Missed {} events", missed),
            Ok(event) => println!("{:?}", event),
            Err(ClientError::GcRefused { code, message }) => eprintln!("Refused: {} ({})", message, code),
            Err(error) => return Err(error),
        }
    }
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
    let (handle, _events, task) = client.spawn();

    let work = async {
        let mut cs2 = Cs2::connect(&handle).await?;
        println!(
            "CS2's game coordinator welcomed the account: {} items",
            cs2.inventory().len()
        );
        for item in cs2.inventory().iter().take(10) {
            println!(
                "  {}: def index {}, paint index {:?}, pattern {:?}, float {:?}, name {:?}",
                item.id(),
                item.def_index(),
                item.paint_index(),
                item.pattern(),
                item.float(),
                item.custom_name()
            );
        }
        let stats = cs2.request_matchmaking_stats().await?;
        println!("Matchmaking stats:");
        print_profile(&stats);
        ask(&mut cs2).await?;
        print_events(&mut cs2).await
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
