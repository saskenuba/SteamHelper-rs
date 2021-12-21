# Steam-Mobile

This library provides vital Steam Mobile Authenticator functionality, especially
useful for building Steam trading bots, but not limited to, managing your Steam
Account outside of the mobile app, which certainly is bad for accepting
market/trade confirmations.

This library aims to be robust across various errors that can happen with Steam
Network. At this moment, steam-auth implements some retry strategies to log in
and parental control to make the experience more consistent, while passing up
the chain errors that can't be retried.

## Usage

You can use the CLI (enabled by default) to generate TOTP codes, add and remove
Authenticators, and at a later moment accept/deny confirmations.

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
  * Generate login codes for Shared Secrets;
  * Add SteamGuard authenticator, saving maFile(secrets);

## Planned
* Remove SteamGuard from account;
* More CLI functionality, such as accept or deny market confirmations;
