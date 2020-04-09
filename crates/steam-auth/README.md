This library provides vital Steam Mobile Authenticator functionality, specially useful for building Steam trading bots.

You can use the CLI (enabled by default) to generate TOTP codes - and other stuff later on - from steam's shared secret.

If you don't want to use the CLI, and are only interested in the library disable it by adding to your Cargo.toml:

```
[dependencies.steam-auth]
version = *
default-features = false
```

# Features being implemented now
* Login to a user account (and API key retrieval)
* Fetch, accept, and deny mobile confirmations

## Implemented
* Generate login codes for a given Shared Secret - CLI

# Planned Features
* Link and activate a new mobile authenticator to a user account after logging in
* Remove itself from an account
