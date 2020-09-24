 # Steam-Web-API

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
steam-web-api = { git = "https://github.com/saskenuba/SteamHelper-rs.git", branch = "master" }

```

Or if you want the blocking client:

```toml
steam-web-api = { git = "https://github.com/saskenuba/SteamHelper-rs.git", branch = "master", default-features = false, features = ["blocking"] }
```

Then in your `lib.rs` or `main.rs` file add:

```rust
use steam_web_api::{Executor, Github};
```

 ``` rust
use steam_web_api::{Executor, Github};

// if using blocking client
// use steam_web_api::blocking::{Executor, Github};


#[tokio::main]
async fn main() -> Result<()> {
    let client = SteamAPI::new(std::env!("STEAM_API"));

    // You choose between the already structured response
    let response: steam-web-api::response_types::GetPlayerBansBase = client
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

// Not all endpoints have the structured endpoint response. You can contribute!
 ```
