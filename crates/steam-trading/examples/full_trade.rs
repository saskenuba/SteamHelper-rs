//! Two bots, A and B, and one CS2 item: from the login to where the item is after the trade.
//!
//! A logs in to the websites with steam-mobile. B logs on to Steam (CM) with steam-client, with its password and a
//! Steam Guard code from its maFile, and gets its web session through that (steam-mobile's `login_with_client`, no
//! second password login; the session renews itself through the client): the trade needs B's session with CS2's game
//! coordinator, which tells where the received item went (since 2026-04 Steam's trade history doesn't). Both accounts
//! must own CS2 (its free license).
//!
//! A [`SteamTradeManager`] needs nothing but the logged-in authenticator: it sends the session's access token to the
//! Web API, and acknowledges Steam's trade protection notice by itself before its first confirmation.
//!
//! - `inspect` (the default, read-only): both bots' CS2 inventories ([`InventoryContext::CS2`] and
//!   [`InventoryContext::CS2_TRADE_PROTECTED`]), with pattern, wear and lock; then their last trades.
//! - `trade`: A's tradable items, to pick one.
//! - `trade <assetid>`: A sends B that item (to B's trade link, from `my_trade_link`); B reads its pattern and wear
//!   from A's inventory, as a bot does before a trade, reads the offer among its received ones (`offers`) and accepts
//!   it with
//!   `accept_and_find_items`. Ends on one line:
//!   `A's <old id> → B's <new id> · found by <how> · unlocks <time> (in Nd Nh)`.
//!
//! ```sh
//! cargo run -p steam-trading --features cs2 --example full_trade
//! cargo run -p steam-trading --features cs2 --example full_trade -- trade
//! cargo run -p steam-trading --features cs2 --example full_trade -- trade 24234497846
//! ```
//!
//! Reads `A_MAFILE`, `A_PASSWORD_FILE`, `B_MAFILE`, `B_PASSWORD_FILE` (defaults `~/.steam-test/a.maFile` …). The
//! password is never read from the maFile.
//!
//! **Sending and accepting ask first** (type yes); an accepted trade can't be undone. `YES=send,accept` answers yes
//! without asking, for runs without a terminal. Prints no secret (no trade link token either), but asset and trade ids.

use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use steam_client::client::SteamClient;
use steam_client::client::User;
use steam_client::errors::ClientError;
use steam_client::gc::cs2::Cs2;
use steam_client::handle::Events;
use steam_client::handle::SteamHandle;
use steam_client::handle::SteamTask;
use steam_mobile::user::PresentMaFile;
use steam_mobile::user::SteamUser;
use steam_mobile::Authenticated;
use steam_mobile::MobileAuthFile;
use steam_mobile::SteamAuthenticator;
use steam_trading::Asset;
use steam_trading::AssetId;
use steam_trading::Inventory;
use steam_trading::InventoryContext;
use steam_trading::InventoryItem;
use steam_trading::Located;
use steam_trading::OfferQuery;
use steam_trading::OfferRequest;
use steam_trading::SteamID;
use steam_trading::SteamTradeManager;
use steam_trading::TradeQuery;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Web = SteamAuthenticator<Authenticated, PresentMaFile>;

/// CS2's inventory contexts, as `inspect` prints them.
const CS2_CONTEXTS: [(InventoryContext, &str); 2] = [
    (InventoryContext::CS2, "tradable"),
    (InventoryContext::CS2_TRADE_PROTECTED, "trade protected"),
];
/// How many of an account's last trades `inspect` prints.
const RECENT_TRADES: u32 = 10;
/// The message of the offer A sends.
const OFFER_MESSAGE: &str = "full_trade example";

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

/// The maFile, account name and password at `<prefix>_MAFILE` and `<prefix>_PASSWORD_FILE`.
fn credentials(prefix: &str) -> Result<(MobileAuthFile, String, String)> {
    let lower = prefix.to_lowercase();
    let mafile = MobileAuthFile::from_disk(path(&format!("{prefix}_MAFILE"), &format!("{lower}.maFile"))?)?;
    let account = mafile.account_name.clone().ok_or("the maFile has no account_name")?;
    let password = fs::read_to_string(path(&format!("{prefix}_PASSWORD_FILE"), &format!("{lower}.password"))?)?;
    let password = password.trim_end_matches(['\r', '\n']).to_string();
    Ok((mafile, account, password))
}

/// Logs the account of `prefix` in to the websites, with steam-mobile.
async fn login(prefix: &str) -> Result<Web> {
    let (mafile, account, password) = credentials(prefix)?;
    let user = SteamUser::new(account, password).with_mafile(mafile);
    let web = SteamAuthenticator::new(user).login().await?;
    println!("{prefix}: logged in as {}", web.steam_id().to_steam64());
    Ok(web)
}

/// An account logged on to Steam (CM), and to the websites through that session.
struct LoggedOn {
    handle: SteamHandle<User>,
    /// Kept so that the client keeps its events, unread.
    _events: Events,
    task: SteamTask,
    /// Its trades, whose manager owns its web session.
    trades: SteamTradeManager,
}

/// Logs the account of `prefix` on to Steam (CM), with its password and a Steam Guard code from its maFile, then in
/// to the websites through that session.
async fn log_on(prefix: &str) -> Result<LoggedOn> {
    let (mafile, account, password) = credentials(prefix)?;
    let mut client = SteamClient::connect().await?;
    let mut session = client.begin_auth_session(&account, &password).await?;
    session.submit_device_code(&mafile.auth_code().await?).await?;
    let (account_name, token) = session.poll().await?.into_parts();
    let client = client.login_with_refresh_token(&account_name, token).await?;
    let (handle, events, task) = client.spawn();

    // these dependencies are weird.
    // What does steam-client depend on steam authenticator?
    // it feels that steam authenticator should depend on steam client for a web login, and if it has the steam client inside, steam trading can also do things with it.
    // everything internally, so we don't need to call it here, on its side, see?
    let user = SteamUser::new(account, password).with_mafile(mafile);
    let web = SteamAuthenticator::new(user).login_with_client(handle.clone()).await?;
    println!(
        "{prefix}: logged on as {}, and to the websites",
        web.steam_id().to_steam64()
    );
    Ok(LoggedOn {
        handle,
        _events: events,
        task,
        trades: SteamTradeManager::new(web),
    })
}

/// Asks `question`, unless `YES` names `step`; true for "yes".
fn confirm(step: &str, question: &str) -> Result<bool> {
    let preapproved = std::env::var("YES").unwrap_or_default();
    if preapproved.split(',').any(|yes| yes.trim() == step) {
        println!("{question} Yes (YES={preapproved}).");
        return Ok(true);
    }
    print!("{question} Type yes to go on: ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(answer.trim() == "yes")
}

/// A Unix time, and how far away it is.
fn when(unix_time: i64) -> String {
    let time = UNIX_EPOCH + Duration::from_secs(unix_time.max(0) as u64);
    match time.duration_since(SystemTime::now()) {
        Ok(left) => {
            let hours = left.as_secs() / 3600;
            format!("{unix_time} (in {}d {}h)", hours / 24, hours % 24)
        }
        Err(_) => format!("{unix_time} (over)"),
    }
}

/// One line for an item: id, name, pattern, wear, and its lock if it has one.
fn describe(item: &InventoryItem<'_>) -> String {
    let lock = item
        .owner_descriptions()
        .into_iter()
        .find(|text| text.contains("trade-protected"))
        .map(|text| format!(" | {}", text.trim_start_matches("⇆ ")))
        .unwrap_or_default();
    let pattern = item
        .pattern()
        .map(|pattern| format!(" pattern {pattern}"))
        .unwrap_or_default();
    let wear = item.wear().map(|wear| format!(" wear {wear:.6}")).unwrap_or_default();
    format!("{:>12}  {}{pattern}{wear}{lock}", item.assetid, item.market_hash_name)
}

/// The items of `inventory` that pass `keep`, one line each.
fn lines(inventory: &Inventory, keep: impl Fn(&InventoryItem<'_>) -> bool) -> Vec<String> {
    inventory
        .pages()
        .flat_map(|page| {
            page.items()
                .filter(|item| keep(item))
                .map(|item| describe(&item))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Both CS2 contexts of an account, printed.
async fn print_inventory(name: &str, trades: &SteamTradeManager) -> Result<()> {
    for (context, label) in CS2_CONTEXTS {
        let items = lines(&trades.own_inventory(context).await?, |_| true);
        println!("{name}: {} {label}", items.len());
        for item in items {
            println!("  {item}");
        }
    }
    Ok(())
}

/// An account's last [`RECENT_TRADES`] trades, failed ones too, with what moved and when it unlocks.
async fn print_recent_trades(name: &str, trades: &SteamTradeManager) -> Result<()> {
    let query = TradeQuery {
        max: RECENT_TRADES,
        include_failed: true,
    };
    println!("{name}: last trades");
    for trade in trades.trades(query).await? {
        let lock = trade.protected_until.map(when).unwrap_or_else(|| "none".to_string());
        println!("  trade {} {:?}, lock ends {lock}", trade.id, trade.status);
        let received = trade.received.iter().map(|item| ("received", item));
        let given = trade.given.iter().map(|item| ("gave", item));
        for (side, item) in received.chain(given) {
            match item.new_asset {
                Some(new) => println!("    {side} {}, now {}", item.asset.id, new.id),
                None => println!("    {side} {}, no new id from Steam (CS2 since 2026-04)", item.asset.id),
            }
        }
    }
    Ok(())
}

/// How `accept_and_find_items` told the item apart.
fn how(located: Located) -> &'static str {
    match located {
        Located::Exact(_) => "pattern and wear",
        Located::AnyOfKind(_) => "kind",
        Located::NotFound => "nothing",
    }
}

/// B's session with CS2's game coordinator, which sent B's inventory.
async fn connect_cs2(b: &LoggedOn) -> Result<Cs2> {
    let mut cs2 = match Cs2::connect(&b.handle).await {
        Ok(cs2) => cs2,
        Err(ClientError::GcTimeout(_)) => return Err("CS2's game coordinator didn't answer B: does B own CS2?".into()),
        Err(error) => return Err(error.into()),
    };
    let items = cs2
        .inventory()?
        .ok_or("CS2's game coordinator sent B no inventory")?
        .len();
    println!("B: CS2's game coordinator has {items} items");
    Ok(cs2)
}

/// A sends `assetid` to B, B accepts; then where the item went, and when it unlocks.
async fn trade(
    a_trades: &SteamTradeManager,
    a_id: SteamID,
    b: &LoggedOn,
    b_trades: &SteamTradeManager,
    assetid: AssetId,
) -> Result<()> {
    let a_items = a_trades.own_inventory(InventoryContext::CS2).await?;
    let name = a_items
        .pages()
        .find_map(|page| {
            page.items()
                .find(|item| item.assetid == assetid && item.tradable)
                .map(|item| item.market_hash_name.to_string())
        })
        .ok_or("not one of A's tradable CS2 items: run `trade` to list them")?;

    // before sending anything: B must be able to tell where the item goes, from A's copy as B reads it (the offer
    // carries no pattern or wear)
    let mut cs2 = connect_cs2(b).await?;
    let fingerprints = b_trades.inventory(a_id, InventoryContext::CS2).await?.fingerprints();

    let b_link = b_trades.my_trade_link().await?;
    let offer = OfferRequest::to(b_link)
        .give(Asset::new(InventoryContext::CS2, assetid))
        .message(OFFER_MESSAGE);
    if !confirm("send", &format!("A sends B {name} ({assetid})."))? {
        return Ok(());
    }
    // trade protection is acknowledged before the first confirmation, by the manager
    let offer_id = a_trades.send(&offer).await?;
    println!("A: sent and confirmed offer {offer_id}");

    if !confirm("accept", &format!("B accepts offer {offer_id}; this can't be undone."))? {
        a_trades.cancel_offer(offer_id).await?;
        println!("A: canceled offer {offer_id}");
        return Ok(());
    }
    // B reads the offer, as a bot does before accepting it: from `GetTradeOffers`, which was seen live
    let offer = b_trades
        .offers(OfferQuery::ACTIVE_RECEIVED)
        .await?
        .into_iter()
        .find(|offer| offer.id == offer_id)
        .ok_or("B: Steam lists no such offer")?;
    let accepted = b_trades.accept_and_find_items(&offer, &fingerprints, &mut cs2).await?;
    match accepted.trade_id {
        Some(trade_id) => println!("B: accepted offer {offer_id}: trade {trade_id}"),
        None => println!("B: accepted offer {offer_id} after a confirmation; Steam gave no trade id"),
    }

    let received = accepted.received.first().ok_or("the offer had no item for B")?;
    let Some(new_id) = received.located.assetid() else {
        println!("A's {assetid} → B: not in B's game coordinator inventory in time");
        return Ok(());
    };
    // the item's own lock from the game coordinator (live, its `tradable_after` is the trade's `time_settlement`),
    // else the trade's from Steam's trade history
    let item_lock = cs2
        .inventory()?
        .and_then(|inventory| inventory.get(new_id.0))
        .and_then(|item| item.tradable_after())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|since| since.as_secs() as i64);
    let lock = match (item_lock, accepted.trade_id) {
        (Some(end), _) => Some(end),
        (None, Some(trade_id)) => b_trades.trade(trade_id).await?.and_then(|trade| trade.protected_until),
        (None, None) => None,
    };
    let unlocks = lock.map(when).unwrap_or_else(|| "not locked".to_string());
    println!(
        "A's {assetid} → B's {new_id} · found by {} · unlocks {unlocks}",
        how(received.located)
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (a, b) = tokio::try_join!(login("A"), log_on("B"))?;

    let result = run(&args, a, &b).await;

    let LoggedOn { handle, task, .. } = b;
    match handle.log_off().await {
        Ok(()) | Err(ClientError::Closed) => {}
        Err(error) => eprintln!("B: log off: {error}"),
    }
    println!("B: stopped: {:?}", task.await);
    result
}

async fn run(args: &[String], a: Web, b: &LoggedOn) -> Result<()> {
    // the authenticators are all they need: access token for the Web API, cookies for the rest; each manager owns its
    // account's
    let a_id = a.steam_id();
    let a_trades = SteamTradeManager::new(a);
    let b_trades = &b.trades;

    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] | ["inspect"] => {
            print_inventory("A", &a_trades).await?;
            print_inventory("B", b_trades).await?;
            print_recent_trades("A", &a_trades).await?;
            print_recent_trades("B", b_trades).await?;
        }
        ["trade"] => {
            let a_items = a_trades.own_inventory(InventoryContext::CS2).await?;
            println!("A's tradable items: run again with `trade <assetid>`");
            for item in lines(&a_items, |item| item.tradable) {
                println!("  {item}");
            }
        }
        ["trade", assetid] => trade(&a_trades, a_id, b, b_trades, assetid.parse()?).await?,
        _ => return Err("usage: full_trade [inspect | trade [<assetid>]]".into()),
    }
    Ok(())
}
