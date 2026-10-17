# SteamID-Parser

[![Crate version on crates.io](https://img.shields.io/crates/v/steamid-parser)](https://crates.io/crates/steamid-parser)
[![Crate documentation on docs.rs](https://img.shields.io/docsrs/steamid-parser)](https://docs.rs/steamid-parser)
![Crate license](https://img.shields.io/crates/l/steamid-parser)

Parse, convert and de/serialize Steam IDs, such as Steam2, Steam3 and Steam64.

## Installation

To use it, add this to your Cargo.toml:

```toml
[dependencies.steamid-parser]
version = "^1"
```

## Usage

```rust
use steamid_parser::SteamID;

let steamid: SteamID = "STEAM_0:0:12284834".parse()?;
assert_eq!(steamid.to_steam64(), 76561197984835396);
assert_eq!(steamid.to_steam3_string(), "[U:1:24569668]");

// or find one inside a text, such as a profile url
let found = SteamID::find("https://steamcommunity.com/profiles/76561197984835396");
assert_eq!(found, Some(steamid));
```

With the `serialize` feature (enabled by default), a `SteamID` serializes as
Steam64, and deserializes from a number or from a string in any of the formats
above.

Check out the docs for more.
