# Steam-Mobile

[![Crate version on crates.io](https://img.shields.io/crates/v/steam-mobile)](https://crates.io/crates/steam-mobile)
[![Crate documentation on docs.rs](https://img.shields.io/docsrs/steam-mobile)](https://docs.rs/steam-mobile)
![Crate license](https://img.shields.io/crates/l/steam-mobile)


This library provides vital Steam Mobile Authenticator functionality, especially
useful for building Steam trading bots, but not limited to them, such as
managing your Steam Account outside of the mobile app, which is certainly bad at
accepting market/trade confirmations.

This library aims to be robust across various errors that can happen with the Steam
Network. At this moment, steam-mobile implements some retry strategies for
logging in and parental control to make the experience more consistent, while passing up
the chain errors that can't be retried.

## Usage

You can use the CLI (enabled by default) to generate TOTP codes, add
authenticators, and at a later moment list, accept or deny confirmations.

Most commands need your maFile, which you can pass with `--mafile` or the
`STEAM_MAFILE` environment variable. The account name is read from the maFile,
but you can override it with `--account`. Your password is read from
`STEAM_PASSWORD`, or asked for if it isn't set, so it never ends up in your
shell history.

```sh
steam-mobile code
steam-mobile confirmations list
steam-mobile confirmations accept 13385072611
steam-mobile confirmations accept --trade-offer 4009687284
steam-mobile confirmations accept --kind family
steam-mobile confirmations deny --all
steam-mobile --account myaccount authenticator add --out ~/steam
steam-mobile authenticator status
```

Confirmations can be picked by their id (as shown by `list`), by trade offer,
by kind, such as `family` for Steam Family invites and joins, or all at once.
Be careful with `--all`, since it also acts on account recovery and phone
number changes!

When adding an authenticator, Steam sends you an activation code by SMS if your
account has a phone, or by email if it doesn't, so a phone number is optional.
With `--out`, your maFile is saved as `<account>.maFile.pending` before
anything is finalized, and renamed to `<account>.maFile` once it is. If
something goes wrong in between, you can finish it later with
`steam-mobile --mafile <account>.maFile.pending authenticator finalize`.

If your maFile is missing its device ID, which some tools leave out,
`authenticator status` recovers it from Steam and offers to save it.

If you don't want to use the CLI, and are only interested in the library for
automation, you can disable it by adding this to your Cargo.toml:


```toml
[dependencies.steam-mobile]
version = "^0.6"
default-features = false
```

## Documentation

Still to do. You can find something if you generate the docs, but they are quite
outdated at the moment.

# Features

Below, you can find a list of implemented and planned features:

## Implemented

### Library ###

 * Log in to a user account with SteamGuard enabled, captchas and parental control;
 * Log in through a Steam client logged on with steam-client-lib (feature `cm`), which then renews the session by
   itself (examples `web_session` and `web_session_bot`);
 * Add, finalize and remove SteamGuard authenticators, with or without a phone number;
 * Generate secrets;
 * Recover a maFile's missing device ID;
 * Register and revoke the Web API key;
 * Accept, deny and fetch mobile confirmations;

### CLI ###
  * Generate login codes from your maFile;
  * Add a SteamGuard authenticator, with or without a phone number, saving the maFile (secrets);
  * Show SteamGuard status, and recover a maFile's missing device ID;
  * List, accept and deny mobile confirmations, by id, trade offer or kind;

## Planned
* Remove SteamGuard from an account through the CLI;
* Interactive picker for confirmations, and JSON output for scripting;
