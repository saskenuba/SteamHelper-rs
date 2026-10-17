# SteamHelper-rs

SteamHelper is (in the future) a modular Rust alternative to popular libraries
such as C# [SteamRE/SteamKit](https://github.com/SteamRE/SteamKit), node.js
[DoctorMcKay/node-steam-client](https://github.com/DoctorMcKay/node-steam-client),
and Python [ValvePython/steam](https://github.com/ValvePython/steam), to enable
interaction with the Steam Network through an easy-to-follow API.

It can be used to create bots and automate profiles; the possibilities are endless.

The library needs contributors. Check issues that need help and send those PRs
in! To learn more about how Steam works, check
[here](https://github.com/saskenuba/SteamHelper-rs/blob/master/docs/dev/README.md).

## Crates:

### Stable:
- **Steam Trading**: Create/Accept/Deny trade offers and confirm them through mobile;
- **Steam Mobile**: Generate mobile 2FA codes, add authenticators (with or
  without a phone number), list/accept/deny confirmations (library/cli);
- **Tappet**: Typed wrapper around the Steam Web API. Allows late injection of
  API key and client reuse. Ergonomic;
- **Steam Language**: Steam enums, message ids and messages, such as EResult,
  EUniverse and EMsg, generated from SteamKit;
- **Steam Protobuf**: Rust definitions of Steam Protocol Buffers, one feature
  per proto;
- **SteamID Parser**: De/serialize Steam IDs from and to Steam2, Steam3 and
  Steam64;
- **Steam TOTP**: Generate Steam TOTP codes, nothing more;

### In Progress:
- **Steam Client**: Same functionality as the desktop client: go online, answer
  messages, etc. Anonymous logon works, with zero-alloc deserialization.
  Account logon, and handing its web session to Steam Mobile/Trading, are on
  the way. Still WIP;

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in SteamHelper by you shall be licensed as MIT, without any
additional terms or conditions.
