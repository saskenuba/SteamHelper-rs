//! Asks for free licenses: logs on with a saved refresh token, sends `ClientRequestFreeLicense` for the apps given,
//! prints what Steam granted, then the license lists Steam sends, and logs off.
//!
//! ```sh
//! STEAM_ACCOUNT=<account name> RUST_LOG=steam_client=info cargo run -p steam-client-lib --example free_license -- 440
//! ```
//!
//! Reads:
//! - `STEAM_ACCOUNT`: the account name.
//! - `STEAM_REFRESH_TOKEN_FILE` (`~/.steam-test/refresh_token`): the refresh token the `login` example saved.
//! - The app ids as arguments, such as 440 (Team Fortress 2), a free game.
//! - `LICENSE_SECONDS` (5): how long to wait for license lists after the answer.
//! - `BY_HAND=1`: sends the request through `RequestFreeLicenseByHand` (below) instead of the crate's marker.
//!
//! Expect `Granted apps [440], packages [...]`, or nothing granted for an app that isn't free or that the account
//! already owns (Steam still answers `OK`). Steam sends the account's license list (`ClientLicenseList`) after
//! logon, and again after a grant, with one more license. Prints no token.
//!
//! **Not verified live.** Granting a license changes the account: it then owns the app. Steam limits how often an
//! account may ask (`RateLimitExceeded`).
//!
//! # A job message the crate doesn't know
//!
//! `call_job_message(request)` finds the crate's own job messages from the request (`steam_client::job::messages`).
//! Another one needs a marker type of yours that implements `JobMessage`, sent with `call_job::<Marker>(request)`:
//! see `RequestFreeLicenseByHand` below, the same message declared as a caller would (`BY_HAND=1` uses it).

use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use steam_client::auth::RefreshToken;
use steam_client::client::SteamClient;
use steam_client::events::Event;
use steam_client::job::JobMessage;
use steam_language_gen::generated::enums::EMsg;
use steam_protobuf::protobufs::steammessages_clientserver::CMsgClientLicenseList;
use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgClientRequestFreeLicense;
use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgClientRequestFreeLicenseResponse;
use tokio::time::timeout_at;
use tokio::time::Instant;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `ClientRequestFreeLicense`, declared as a caller declares a job message the crate doesn't know. The crate's own
/// is `steam_client::job::messages::RequestFreeLicense`.
enum RequestFreeLicenseByHand {}

impl JobMessage for RequestFreeLicenseByHand {
    const REQUEST: EMsg = EMsg::ClientRequestFreeLicense;
    const RESPONSE: EMsg = EMsg::ClientRequestFreeLicenseResponse;
    type Request = CMsgClientRequestFreeLicense;
    type Response = CMsgClientRequestFreeLicenseResponse;

    // the call fails unless it is OK
    fn result(response: &Self::Response) -> Option<i32> {
        Some(response.eresult() as i32)
    }
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

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let account = std::env::var("STEAM_ACCOUNT").map_err(|_| "set STEAM_ACCOUNT to the account name")?;
    let token_path = path("STEAM_REFRESH_TOKEN_FILE", "refresh_token")?;
    let token = fs::read_to_string(&token_path).map_err(|_| "no saved refresh token: run the login example first")?;
    let token = RefreshToken::new(token.trim().to_string())?;
    let app_ids = std::env::args()
        .skip(1)
        .map(|arg| arg.parse::<u32>())
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| "the arguments are app ids, such as 440")?;
    if app_ids.is_empty() {
        return Err("give the app ids to ask for, such as 440".into());
    }
    let license_seconds = match std::env::var("LICENSE_SECONDS") {
        Ok(seconds) => seconds.parse()?,
        Err(_) => 5,
    };

    let mut client = SteamClient::connect()
        .await?
        .login_with_refresh_token(&account, token)
        .await?;
    println!("Logged on as {}", client.steam_id().to_steam64());

    let mut request = CMsgClientRequestFreeLicense::new();
    request.appids = app_ids.clone();
    let answer = if std::env::var_os("BY_HAND").is_some() {
        client.call_job::<RequestFreeLicenseByHand>(request).await
    } else {
        // the crate's `RequestFreeLicense`, found from the request
        client.call_job_message(request).await
    };
    match &answer {
        Ok(granted) if granted.granted_appids.is_empty() && granted.granted_packageids.is_empty() => {
            println!("Nothing granted for {:?}: not free, or already owned", app_ids)
        }
        Ok(granted) => println!(
            "Granted apps {:?}, packages {:?}",
            granted.granted_appids, granted.granted_packageids
        ),
        // such as `RateLimitExceeded`
        Err(error) => eprintln!("Couldn't ask for {:?}: {}", app_ids, error),
    }

    // the lists sent since logon, kept as events, then those that come within `LICENSE_SECONDS`
    let deadline = Instant::now() + Duration::from_secs(license_seconds);
    while let Ok(event) = timeout_at(deadline, client.next_event()).await {
        match event? {
            Event::Message(message) if message.emsg() == EMsg::ClientLicenseList => {
                let list = message.parse_proto::<CMsgClientLicenseList>()?;
                println!("License list: {} licenses", list.licenses().len());
            }
            Event::LoggedOff(result) => return Err(format!("Steam logged us off: {:?}", result).into()),
            _ => {}
        }
    }

    client.log_off().await?;
    println!("Logged off");
    answer?;
    Ok(())
}
