# SteamTrading

This library provides functionality regarding Steam Trade.

You can create accept, deny and create new trade offers, and in the future
automatically keep track of statuses changes of offers through auto managed API
calls.

This will include getting new Assetids after the trade is complete among other
features.

To use it, add this to your Cargo.toml:

```toml
[dependencies.steam-trading]
version = "*"
```


## Implemented
* Abstractions of Trade Offers, and assets to make offers easy to use;
* Create and send a new trade offer;

# Planned Features
* Accept and Deny trade offers;
* Keep track of statuses changes;
