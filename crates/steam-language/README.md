# steam-language

[![Crate version on crates.io](https://img.shields.io/crates/v/steam-language)](https://crates.io/crates/steam-language)
[![Crate documentation on docs.rs](https://img.shields.io/docsrs/steam-language)](https://docs.rs/steam-language)
![Crate license](https://img.shields.io/crates/l/steam-language)

Steam's protocol language: enums, message ids and messages, generated from
[SteamKit](https://github.com/SteamRE/SteamKit)'s SteamLanguage files.

Each part is behind a feature, so a crate only builds what it uses:

| Feature | Contents | Extra dependencies |
|---|---|---|
| none | `enums`: `EResult`, `EAccountType`, `EUniverse`, `ETradeOfferState`, ... | none |
| `emsg` | `emsg`: `EMsg` message ids, and helpers to read them from packets | `arrayref` |
| `messages` | `headers`, `messages` and their traits. Enables `emsg` | `steam-protobuf`, `bincode` |

The `messages` feature is **unstable**: it is not covered by semver and can change in minor releases.
`enums` and `emsg` follow semver.

## Installation

```toml
[dependencies]
# enums only
steam-language = "1"
# or with messages
steam-language = { version = "1", features = ["messages"] }
```
