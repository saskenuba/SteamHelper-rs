# Steam-Trading

[![Crate version on crates.io](https://img.shields.io/crates/v/steam-trading)](https://crates.io/crates/steam-trading)
[![Crate documentation on docs.rs](https://img.shields.io/docsrs/steam-trading)](https://docs.rs/steam-trading)
![Crate license](https://img.shields.io/crates/l/steam-trading)


This library provides functionality regarding Steam Trade.

You can create accept, deny and create new trade offers, and in the future
automatically keep track of statuses changes of offers through auto managed API
calls.

This will include getting new Assetids after the trade is complete among other
features.

To use it, add this to your Cargo.toml:

```toml
[dependencies.steam-trading]
version = "^0.4"
```

## Implemented
* Abstractions of Trade Offers, and assets to make offers easy to use;
* Create and send a new trade offer;
* Accept and Deny trade offers;
