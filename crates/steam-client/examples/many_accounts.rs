//! Runs many clients in one process: logs on N anonymous clients, spawns them all, idles, and shows what they cost in
//! memory, then logs them all off.
//!
//! Run with `cargo run -p steam-client-lib --example many_accounts -- 5`, or set `ACCOUNTS`: 5 clients by default, 20
//! at most, to be gentle with Steam. They idle 60 s; set `IDLE_SECONDS` to change it. Expect a `Logged on` line per
//! client, then the process's resident memory (RSS, from `ps`) before, after the first client and after them all, and
//! per client, then `Logged off` per client. A client that fails to connect or log on stops the setup; those
//! already logged on still log off, as do all of them if one fails to.
//!
//! The first client costs the most: it also brings up what every client shares, such as the HTTP client that fetches
//! Steam's server list. So the cost of each further one is the second figure. RSS is the whole process, allocator
//! and runtime included; `tests/memory.rs` counts the crate's own allocations exactly.

use std::process::Command;
use std::time::Duration;

use steam_client::client::SteamClient;
use steam_client::errors::ClientError;
use steam_client::events::Event;

const MAX_ACCOUNTS: usize = 20;

/// The process's resident memory, in KiB, as `ps` reports it (macOS and Linux).
fn rss_kib() -> u64 {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("ps prints a number")
}

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    env_logger::init();
    let accounts: usize = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("ACCOUNTS").ok())
        .and_then(|accounts| accounts.parse().ok())
        .unwrap_or(5);
    assert!((1..=MAX_ACCOUNTS).contains(&accounts), "1 to {} accounts", MAX_ACCOUNTS);
    let idle = Duration::from_secs(
        std::env::var("IDLE_SECONDS")
            .ok()
            .and_then(|seconds| seconds.parse().ok())
            .unwrap_or(60),
    );

    let before = rss_kib();
    let mut after_first = before;
    let mut endpoint: Option<String> = None;
    let mut clients = Vec::new();
    // the first error, once every client logged off
    let mut failed = None;
    for account in 0..accounts {
        // the server list is fetched once: the others connect to the same server
        let connected = match &endpoint {
            None => SteamClient::connect().await,
            Some(endpoint) => SteamClient::connect_to(endpoint).await,
        };
        let client = match connected {
            Ok(client) => client,
            Err(error) => {
                eprintln!("{:>2}: Couldn't connect: {}", account, error);
                failed = Some(error);
                break;
            }
        };
        endpoint = Some(client.endpoint().to_string());
        let client = match client.login_anonymous().await {
            Ok(client) => client,
            Err(error) => {
                eprintln!("{:>2}: Couldn't log on: {}", account, error);
                failed = Some(error);
                break;
            }
        };
        println!("{:>2}: Logged on as {}", account, client.steam_id());

        let (handle, mut events, task) = client.spawn();
        // every client's events are read, in a task of their own, until the client stops
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                if !matches!(event, Event::Message(_)) {
                    println!("{:>2}: {:?}", account, event);
                }
            }
        });
        clients.push((handle, task));
        if account == 0 {
            after_first = rss_kib();
        }
    }

    if failed.is_none() {
        println!(
            "{} clients on {}, idle for {:?}",
            accounts,
            endpoint.unwrap_or_default(),
            idle
        );
        tokio::time::sleep(idle).await;
        let after = rss_kib();
        println!(
            "RSS before: {} KiB, after the first client: {} KiB, after all: {} KiB",
            before, after_first, after
        );
        println!(
            "  {} KiB per client, all in",
            after.saturating_sub(before) / accounts as u64
        );
        if accounts > 1 {
            println!(
                "  {} KiB per client after the first",
                after.saturating_sub(after_first) / (accounts as u64 - 1)
            );
        }
    }

    // every client logs off, even if one fails to
    for (account, (handle, task)) in clients.into_iter().enumerate() {
        match handle.log_off().await {
            Ok(()) => println!("{:>2}: Logged off", account),
            Err(error) => {
                eprintln!("{:>2}: Couldn't log off: {}", account, error);
                failed.get_or_insert(error);
            }
        }
        task.await;
    }
    match failed {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
