# Steam-Trading

[![Crate version on crates.io](https://img.shields.io/crates/v/steam-trading)](https://crates.io/crates/steam-trading)
[![Crate documentation on docs.rs](https://img.shields.io/docsrs/steam-trading)](https://docs.rs/steam-trading)
![Crate license](https://img.shields.io/crates/l/steam-trading)


This library provides functionality regarding Steam Trade.

You can create, accept, decline and cancel trade offers, confirming them through
[steam-mobile](https://crates.io/crates/steam-mobile), and in the future
automatically keep track of status changes of offers through auto-managed API
calls.

Steam Guard must be enabled on the account, with mobile confirmations.

To use it, add this to your Cargo.toml:

```toml
[dependencies.steam-trading]
version = "^0.8"
```

## Implemented
* Typed ids (`OfferId`, `TradeId`, `AssetId`) and trade links (`TradeLink`);
* Build an offer (`OfferRequest`), create it and confirm it through mobile (`send`, or the steps `create_offer` and
  `confirm_offers`, which confirms many offers at once);
* Accept (`accept`, or the step `post_accept`), decline and cancel trade offers;
* Read offers (`offers`, following every page Steam returns, and `offer` by id) and trades (`trades`, `trade`), with
  each trade's new asset ids (not for CS2: since ~2026-04 Steam sends none, see feature `cs2`) and the end of its
  trade protection;
* How long a trade with a partner would be held (`trade_hold`);
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
let offer = OfferRequest::to(TradeLink::parse(link)?)
    .give(Asset::new(InventoryContext::CS2, AssetId(1)))
    .message("code 7F3A");
let offer_id = trades.send(&offer).await?;
// the authenticator is still there, for the manager's account
let confirmations = trades.authenticator().fetch_confirmations().await?;
```

## Features
* `cs2`: `accept_and_find_items`, which accepts and finds the new asset ids of the CS2 items received through
  CS2's game coordinator (steam-client-lib), since Steam's trade history no longer gives them for CS2, and
  `find_received_items`, the same for an offer the partner accepted (one this account sent), from the item ids the
  caller already knows. Both take the pattern and wear of the partner's copies, as the caller read them from the
  partner's inventory before the trade (offers carry none); items without them, such as cases, are found by kind.
  Seen live once each (2026-10-22): an item with a fingerprint found exactly, a case found by kind.
  It also turns on steam-mobile's `cm`: one Steam client logon gives the account's CS2
  session and its web session (`SteamAuthenticator::login_with_client`), which then renews itself. The `full_trade`
  example uses it, and `cs2_trade_items` compares the game coordinator's item ids with the web inventory's.
