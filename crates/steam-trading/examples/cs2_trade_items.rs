//! EXPERIMENT (2026-10-21), ugly on purpose: can the CS2 game coordinator tell us the new asset ids of items we
//! received in a trade, now that Steam's trade history doesn't (`new_assetid` missing for CS2 since 2026-04)?
//!
//! Logs on with the password and maFile (CM), logs the same account in to the websites, starts CS2, then prints side
//! by side, read-only:
//! 1. the game coordinator's inventory: id, original_id, origin, tradable_after, float, pattern…;
//! 2. the web inventory, contexts 2 (tradable) and 16 (trade protected): asset ids and the lock text;
//! 3. the recent trades of `GetTradeHistory` (access token): what we received, with the sender's asset id;
//! 4. which ids match: GC id == web asset id? GC original_id == the sender's asset id?
//!
//! With `WATCH_SECONDS=n`, then prints the game coordinator's events for n seconds (e.g. while a trade is accepted).
//!
//! ```sh
//! STEAM_MAFILE=~/x.maFile STEAM_PASSWORD_FILE=~/x.password \
//!   cargo run -p steam-trading --features cs2 --example cs2_trade_items
//! ```

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use steam_client::client::SteamClient;
use steam_client::errors::ClientError;
use steam_client::gc::cs2::Cs2;
use steam_client::gc::cs2::Cs2Event;
use steam_client::gc::cs2::InventoryItem;
use steam_mobile::user::SteamUser;
use steam_mobile::Method;
use steam_mobile::MobileAuthFile;
use steam_mobile::SteamAuthenticator;
use steam_totp::Secret;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Trades started after this (2026-10-01) are printed.
const SINCE: i64 = 1790812800;

fn env(name: &str) -> Result<String> {
    std::env::var(name).map_err(|_| format!("set {name}").into())
}

fn unix(time: Option<SystemTime>) -> Option<u64> {
    time.and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

fn describe(item: &InventoryItem) -> String {
    format!(
        "id {} original_id {:?} def {} paint {:?} pattern {:?} float {:?} origin {:?} quality {:?} rarity {:?} \
         tradable_after {:?} new {} position {:?} storage_unit {:?}",
        item.id(),
        item.raw().original_id,
        item.def_index(),
        item.paint_index(),
        item.pattern(),
        item.float(),
        item.origin(),
        item.quality(),
        item.rarity(),
        unix(item.tradable_after()),
        item.is_new(),
        item.position(),
        item.storage_unit(),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let mafile_path = env("STEAM_MAFILE")?;
    let password = fs::read_to_string(env("STEAM_PASSWORD_FILE")?)?;
    let password = password.trim_end_matches(['\r', '\n']).to_string();
    let mafile_json: serde_json::Value = serde_json::from_str(&fs::read_to_string(&mafile_path)?)?;
    let account = mafile_json["account_name"]
        .as_str()
        .ok_or("no account_name")?
        .to_string();
    let shared_secret = Secret::from_b64(mafile_json["shared_secret"].as_str().ok_or("no shared_secret")?)?;

    // CM logon: password + a code from the maFile, no saved token
    let mut client = SteamClient::connect().await?;
    let mut session = client.begin_auth_session(&account, &password).await?;
    session.submit_shared_secret(shared_secret).await?;
    let (account_name, token) = session.poll().await?.into_parts();
    let client = client.login_with_refresh_token(&account_name, token).await?;
    let steam64 = client.steam_id().to_steam64();
    println!("CM: logged on");
    let (handle, _events, task) = client.spawn();

    let work = async {
        // the websites, through the CM session (no second password login)
        let user =
            SteamUser::new(account.clone(), password.clone()).with_mafile(MobileAuthFile::from_disk(&mafile_path)?);
        let web = SteamAuthenticator::new(user).login_with_client(handle.clone()).await?;
        println!("Web: logged in");

        // does the account own CS2 (its free license)? the game coordinator ignores accounts that don't
        let token = web.access_token().await?.expose_secret().to_string();
        let url = format!(
            "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/?access_token={token}&steamid={steam64}\
             &include_played_free_games=true&include_free_sub=true&skip_unvetted_apps=false&appids_filter[0]=730"
        );
        let response = web.request_custom_endpoint(url, Method::GET, None, None::<()>).await?;
        let status = response.status();
        println!(
            "GetOwnedGames(730): HTTP {status} {}",
            response.text().await?.chars().take(300).collect::<String>()
        );
        if std::env::var("OWNERSHIP_ONLY").is_ok() {
            return Ok(());
        }

        let mut cs2 = Cs2::connect(&handle).await?;
        let inventory = cs2
            .inventory()?
            .ok_or("the game coordinator's welcome had no inventory")?;
        println!("\n== 1. game coordinator inventory: {} items", inventory.len());
        let mut gc_items: BTreeMap<u64, String> = BTreeMap::new();
        let mut gc_by_original: BTreeMap<u64, u64> = BTreeMap::new();
        for item in inventory.iter() {
            println!("  {}", describe(item));
            gc_items.insert(item.id(), describe(item));
            if let Some(original) = item.raw().original_id {
                gc_by_original.insert(original, item.id());
            }
        }

        println!("\n== 2. web inventory");
        let mut web_items: BTreeMap<u64, u32> = BTreeMap::new();
        for context in [2, 16] {
            let url = format!("https://steamcommunity.com/inventory/{steam64}/730/{context}?l=english&count=1000");
            let response = web.request_custom_endpoint(url, Method::GET, None, None::<()>).await?;
            let status = response.status();
            let json: serde_json::Value = serde_json::from_str(&response.text().await?).unwrap_or_default();
            let assets = json["assets"].as_array().cloned().unwrap_or_default();
            let descriptions = json["descriptions"].as_array().cloned().unwrap_or_default();
            println!("  context {context}: HTTP {status}, {} assets", assets.len());
            for asset in &assets {
                let assetid: u64 = asset["assetid"].as_str().unwrap_or("0").parse()?;
                web_items.insert(assetid, context);
                let d = descriptions
                    .iter()
                    .find(|d| d["classid"] == asset["classid"] && d["instanceid"] == asset["instanceid"]);
                let name = d.and_then(|d| d["market_hash_name"].as_str()).unwrap_or("?");
                let lock: Vec<String> = d
                    .and_then(|d| d["owner_descriptions"].as_array())
                    .map(|v| {
                        v.iter()
                            .filter_map(|o| o["value"].as_str())
                            .filter(|s| !s.trim().is_empty())
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                let tradable = d.map(|d| d["tradable"].clone()).unwrap_or_default();
                println!(
                    "    asset {assetid} ({name}) tradable {tradable} in GC: {} lock {lock:?}",
                    gc_items.contains_key(&assetid)
                );
            }
            if let Some(properties) = json.get("asset_properties") {
                println!(
                    "    asset_properties: {}",
                    properties.to_string().chars().take(600).collect::<String>()
                );
            }
        }

        println!("\n== 3. recent trades (GetTradeHistory, access token)");
        let token = web.access_token().await?.expose_secret().to_string();
        let url = format!(
            "https://api.steampowered.com/IEconService/GetTradeHistory/v1/?access_token={token}&max_trades=10&include_failed=true"
        );
        let response = web.request_custom_endpoint(url, Method::GET, None, None::<()>).await?;
        let json: serde_json::Value = serde_json::from_str(&response.text().await?).unwrap_or_default();
        for trade in json["response"]["trades"].as_array().cloned().unwrap_or_default() {
            if trade["time_init"].as_i64().unwrap_or(0) < SINCE {
                continue;
            }
            println!(
                "  trade {} status {} time_init {} time_settlement {}",
                trade["tradeid"], trade["status"], trade["time_init"], trade["time_settlement"]
            );
            for (side, key) in [("received", "assets_received"), ("given", "assets_given")] {
                for asset in trade[key].as_array().cloned().unwrap_or_default() {
                    let old: u64 = asset["assetid"].as_str().unwrap_or("0").parse()?;
                    println!(
                        "    {side} old assetid {old} new_assetid {} \
                         | 4. GC item with original_id == old: {:?}, GC item with id == old: {}",
                        asset["new_assetid"],
                        gc_by_original.get(&old),
                        gc_items.contains_key(&old),
                    );
                }
            }
        }

        println!("\n== 4. ids on one side only");
        for id in gc_items.keys().filter(|id| !web_items.contains_key(id)) {
            println!("  only in GC: {id}");
        }
        for (id, context) in web_items.iter().filter(|(id, _)| !gc_items.contains_key(id)) {
            println!("  only on the web: {id} (context {context})");
        }

        let watch: u64 = std::env::var("WATCH_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if watch > 0 {
            println!("\n== watching the game coordinator for {watch} s");
            let until = tokio::time::Instant::now() + Duration::from_secs(watch);
            loop {
                tokio::select! {
                    event = cs2.next_event() => match event? {
                        Cs2Event::ItemAcquired(item) => println!("  acquired {}", describe(&item)),
                        Cs2Event::ItemChanged { old, new } => println!("  changed\n    old {}\n    new {}", describe(&old), describe(&new)),
                        Cs2Event::ItemRemoved(item) => println!("  removed {}", describe(&item)),
                        other => println!("  {:?}", other),
                    },
                    _ = tokio::time::sleep_until(until) => break,
                }
            }
        }
        Ok::<(), Box<dyn Error>>(())
    };
    let result = work.await;

    match handle.log_off().await {
        Ok(()) | Err(ClientError::Closed) => {}
        Err(error) => eprintln!("log off: {error}"),
    }
    println!("Stopped: {:?}", task.await);
    result
}
