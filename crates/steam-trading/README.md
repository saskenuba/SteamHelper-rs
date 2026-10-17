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
* Fetch trade offers, by id or all of them;
* Get the new Assetids after the trade is complete;
