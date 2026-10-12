# Steam-Mobile

[![Crate version on crates.io](https://img.shields.io/crates/v/steam-mobile)](https://crates.io/crates/steam-mobile)
[![Crate documentation on docs.rs](https://img.shields.io/docsrs/steam-mobile)](https://docs.rs/steam-mobile)
![Crate license](https://img.shields.io/crates/l/steam-mobile)


This library provides vital Steam Mobile Authenticator functionality, especially
useful for building Steam trading bots, but not limited to, managing your Steam
Account outside of the mobile app, which certainly is bad for accepting
market/trade confirmations.

This library aims to be robust across various errors that can happen with Steam
Network. At this moment, steam-auth implements some retry strategies to log in
and parental control to make the experience more consistent, while passing up
the chain errors that can't be retried.

## Usage

You can use the CLI (enabled by default) to generate TOTP codes, add
Authenticators, and at a later moment list, accept or deny confirmations.

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
steam-mobile --account myaccount authenticator add --phone +551112345678 --out ~/steam
```

Confirmations can be picked by their id (as shown by `list`), by trade offer,
by kind, such as `family` for Steam Family invites and joins, or all at once.
Be careful with `--all`, since it also acts on account recovery and phone
number changes!

If you don't want to use the CLI, and are only interested in the library for
automation, you disable it by adding this to your Cargo.toml:


```toml
[dependencies.steam-mobile]
version = "0.2.4"
default-features = false
```

## Documentation

Still todo. You can find something if you generate the docs, but they are quite
outdated at the moment.

# Features

Below, you can find a list of implemented and planned features:

## Implemented

### Library ###

 * Login to a user account with SteamGuard enabled, captchas, parental control;
 * Generate secrets
 * Accept, deny and fetch mobile confirmations;

### CLI ###
  * Generate login codes from your maFile;
  * Add SteamGuard authenticator, saving maFile(secrets);
  * List, accept and deny mobile confirmations, by id, trade offer or kind;

## Planned
* Remove SteamGuard from account;
* Interactive picker for confirmations, and JSON output for scripting;
