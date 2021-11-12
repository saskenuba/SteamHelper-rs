# SteamHelper-rs

SteamHelper is (in the future) a modular Rust alternative to popular libraries
as C# [SteamRE/SteamKit](https://github.com/SteamRE/SteamKit), node.js
[DoctorMcKay/node-steam-client](https://github.com/DoctorMcKay/node-steam-client),
and Python [ValvePython/steam](https://github.com/ValvePython/steam), to enable
interaction with the Steam Network through an easy to follow API.

It can be used to create bots, automate profiles, the possibilities are endless.

The library needs contributors. Check issues that need help and send those PRs
in! To learn more about how Steam works, check
[here](https://github.com/saskenuba/SteamHelper-rs/blob/master/docs/dev/README.md).

## Crates:

### Stable:
- **Steam Trading**: Create/Accept/Deny trade offers and confirm them through mobile;
- **Steam Mobile**: Generate mobile 2FA codes (library/cli), Register 2FA (library/cli);
- **Tappet**: Typed wrapper around Steam Web API. Allows late injection of api
  key and client reuse. Ergonomic;

### Progress Paused:
- **Steam Client**: Same functionality as desktop client, go online, answer to
  messages, etc. Still very WIP;

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in SteamHelper by you, shall be licensed as MIT, without any
additional terms or conditions.
