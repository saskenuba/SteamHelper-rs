# Steam-Trading

[![Crate version on crates.io](https://img.shields.io/crates/v/steam-trading)](https://crates.io/crates/steam-trading)
[![Crate documentation on docs.rs](https://img.shields.io/docsrs/steam-trading)](https://docs.rs/steam-trading)
![Crate license](https://img.shields.io/crates/l/steam-trading)


This library provides functionality regarding Steam Trade.

You can create, accept, deny and cancel trade offers, confirming them through
[steam-mobile](https://crates.io/crates/steam-mobile), and in the future
automatically keep track of status changes of offers through auto-managed API
calls.

Steam Guard must be enabled on the account, with mobile confirmations.

To use it, add this to your Cargo.toml:

```toml
[dependencies.steam-trading]
version = "^0.6"
```

## Implemented
* Abstractions of Trade Offers and assets, to make offers easy to use;
* Create and send a new trade offer, and confirm it through mobile;
* Accept, Deny and Cancel trade offers;
* Decline all received offers at once;
* Fetch trade offers, by id or all of them, following every page Steam returns;
* Get the new Assetids after the trade is complete;
* Web API calls with the session's access token, read from the authenticator for every call: no API key needed
  (with one, Steam leaves CS2 offers and trades out);
* Steam's trade protection notice acknowledged before a manager's first confirmation (Steam refuses to confirm
  trades of trade-protected items until then); `acknowledge_trade_protection` sends it by hand.

## Example
`examples/full_trade.rs` (feature `cs2`) runs a whole trade of one CS2 item between two accounts: A logs in with a
password, B through a Steam client; A creates and confirms an offer to B's trade link, B accepts it, then where the
item went (through CS2's game coordinator) and when its trade protection ends. Its default, `inspect`, only reads
both accounts' CS2 inventories and last trades. It trades for real and asks before sending and before accepting.

```rust,ignore
// one manager per account: it owns the authenticator, and its methods take `&self` (share it with an `Arc`)
let trades = SteamTradeManager::new(authenticator);
let offer_id = trades.create_offer_and_confirm(offer).await?;
// the authenticator is still there, for the manager's account
let confirmations = trades.authenticator().fetch_confirmations().await?;
```

## Features
* `additional-checks` (default): when creating an offer fails, checks whether the partner recently enabled
  Steam Guard (`OfferError::SteamGuardRecentlyEnabled`), and `check_steam_guard_recently_activated`.
* `cs2`: `accept_offer_and_find_items`, which accepts and finds the new asset ids of the CS2 items received through
  CS2's game coordinator (steam-client-lib), since Steam's trade history no longer gives them for CS2, and
  `find_received_items`, the same for an offer the partner accepted (one this account sent), from the item ids the
  caller already knows and the pattern and wear it kept of the partner's copies. Not verified against live Steam yet.
  It also turns on steam-mobile's `cm`: one Steam client logon gives the account's CS2
  session and its web session (`SteamAuthenticator::login_with_client`), which then renews itself. The `full_trade`
  example uses it, and `cs2_trade_items` compares the game coordinator's item ids with the web inventory's.
