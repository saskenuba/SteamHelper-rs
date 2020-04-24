# SteamAuth

This library provides vital Steam Mobile Authenticator functionality,
specially useful for building Steam trading bots.

You can use the CLI (enabled by default) to generate TOTP codes - and
other stuff later on - from steam's shared secret.

If you don't want to use the CLI, and are only interested in the library
disable it by adding to your Cargo.toml:

```toml
[dependencies.steam-auth]
version = "*"
default-features = false
```

<!--# Features being implemented now-->

## Implemented
* Login to a user account;
* Fetch, accept, and deny mobile confirmations;
* Generate login codes for a given Shared Secret - CLI;

# Planned Features
* Link and activate a new mobile authenticator to a user account after
  logging in;
* Remove itself from an account;
* More CLI tools, such as accept or deny market confirmations, add an
  Authenticator;
