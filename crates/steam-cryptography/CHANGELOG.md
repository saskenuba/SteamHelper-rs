# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.0 (2026-10-18)

### Chore

 - <csr-id-11d213edc03f1289fa139d0ebb8b60b4fcde63b8/> publish
   steam-client-lib and steam-cryptography are free on crates.io. Both get
   a license, repository and description, steam-client-lib's path
   dependencies a version, and env_logger a version instead of *, which
   crates.io rejects.

### Refactor (BREAKING)

 - <csr-id-c0354083bed7367bc714b54515b43d9f5e194d64/> the crate is steam-cryptography
   steam-crypto on crates.io is an unrelated 2015 crate, so this one can't
   be published under that name. The directory, package and library are
   steam-cryptography (steam_cryptography::).

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Publish ([`11d213e`](https://github.com/saskenuba/SteamHelper-rs/commit/11d213edc03f1289fa139d0ebb8b60b4fcde63b8))
    - The crate is steam-cryptography ([`c035408`](https://github.com/saskenuba/SteamHelper-rs/commit/c0354083bed7367bc714b54515b43d9f5e194d64))
</details>

