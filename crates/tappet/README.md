# Tappet

[![Crate version on crates.io](https://img.shields.io/crates/v/tappet)](https://crates.io/crates/tappet)
[![Crate documentation on docs.rs](https://img.shields.io/docsrs/tappet)](https://docs.rs/tappet)
![Crate license](https://img.shields.io/crates/l/tappet)

Pure Rust bindings to the [Steam Web
API](https://partner.steamgames.com/doc/webapi) and [XPAW Steam
API](https://steamapi.xpaw.me), where the latter contains some undocumented
albeit useful endpoints of the Steam Web API.

This library was heavily inspired by [Rust Github API
Wrapper](https://github.com/github-rs/github-rs).

Internally, Steam-Web-API uses a reqwest client for simplicity's sake, but this
may change in the future.

## Missing IDE Auto Completion for endpoints

Since this library makes heavy use of procedural macros to generate the strongly
typed bindings, you may not get auto completion for endpoints if your IDE
doesn't support proc-macro completion.

## Lacking features

  * Partner API not available;
  * Not so popular interfaces not available;
  * Responses for all endpoints;

## Getting Started
Add the following to your `Cargo.toml`

```toml
[dependencies]
tappet = "^0.4"

```

Or if you want the blocking client:

```toml
tappet = { version = "^0.4", default-features = false, features = ["blocking"] }
```

Then in your `lib.rs` or `main.rs` file add:

```rust
use tappet::{Executor, SteamAPI};
```


## Example Usage
 ``` rust
use tappet::{Executor, SteamAPI};

// if using blocking client
// use tappet::blocking::{Executor, SteamAPI};


#[tokio::main]
async fn main() -> Result<()> {
    let client = SteamAPI::new(std::env!("STEAM_API"));

    // You choose between the already structured response
    let response: tappet::response_types::GetPlayerBansBase = client
        .get()
        .ISteamUser()
        .GetPlayerBans(vec!["76561197984835396".to_string()])
        .execute_with_response()
        .await?;

    // or the raw response from reqwest
    let response: reqwest::Response = client
         .get()
        .ISteamUser()
        .GetPlayerSummaries(vec!["76561197984835396".to_string()])
        .execute()
        .await?;
}
```


## Reuse with different bots
```rust

// the bot you want to recover pending trade offers
let bot_api_key: &str = "..."

let response: GetTradeOffersResponse = api_client
    .get()
    .IEconService()
    .GetTradeOffers(true, false, u32::MAX, None, None, None, None)
    .inject_custom_key(bot_api_key)
    .execute_with_response()
    .await?;
```

Not all endpoints have the structured endpoint response. You can contribute!
