# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.0 (2024-01-23)

<csr-id-db63677f7d216532c0f072b5dbcf34e2925e6b0e/>

### New Features

 - <csr-id-f80d2d96441ae050348d6a76848492fa8c80a479/> added ProtobufSerialize and ProtobufDeserialize to avoid importing
   external functions for serialization;

### Bug Fixes

 - <csr-id-4fd14a2055fb51ff9224e88576b5efaea4a61424/> changed trait bounds, changed errors
 - <csr-id-f9dcd168a793d1b879ee6f1cf184ee59f5dfde8f/> adjusted steam-protobuf to generate by main function
   later we can create a little script to ease the process

### Other

 - <csr-id-db63677f7d216532c0f072b5dbcf34e2925e6b0e/> updated submodule with latest protobufs

### Other

 - <csr-id-136ceb331a479b538ba363b9448fb14542058eea/> added changelog and prepare for release

### New Features (BREAKING)

 - <csr-id-01e22f1e0f8e5a13f6d67e745ac10e4ea25f29da/> regenerated all steam protobufs, new build.rs

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release over the course of 6 calendar days.
 - 615 days passed between releases.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Added changelog and prepare for release ([`136ceb3`](https://github.com/saskenuba/SteamHelper-rs/commit/136ceb331a479b538ba363b9448fb14542058eea))
    - Changed trait bounds, changed errors ([`4fd14a2`](https://github.com/saskenuba/SteamHelper-rs/commit/4fd14a2055fb51ff9224e88576b5efaea4a61424))
    - Added ProtobufSerialize and ProtobufDeserialize to avoid importing ([`f80d2d9`](https://github.com/saskenuba/SteamHelper-rs/commit/f80d2d96441ae050348d6a76848492fa8c80a479))
    - Adjusted steam-protobuf to generate by main function ([`f9dcd16`](https://github.com/saskenuba/SteamHelper-rs/commit/f9dcd168a793d1b879ee6f1cf184ee59f5dfde8f))
    - Regenerated all steam protobufs, new build.rs ([`01e22f1`](https://github.com/saskenuba/SteamHelper-rs/commit/01e22f1e0f8e5a13f6d67e745ac10e4ea25f29da))
    - Updated submodule with latest protobufs ([`db63677`](https://github.com/saskenuba/SteamHelper-rs/commit/db63677f7d216532c0f072b5dbcf34e2925e6b0e))
</details>

## 0.1.2 (2022-05-17)

<csr-id-690b0d1df9400aa7e23cd613046c6f88f93cb7a9/>

### Other

 - <csr-id-690b0d1df9400aa7e23cd613046c6f88f93cb7a9/> submodules

### Documentation

 - <csr-id-60e3691a305ec8cd3f32fdf5ed68f6b28185b42d/> added CHANGELOG.md, prepare smart-release
 - <csr-id-fb87360214c2f6d1319f467b82b27706ae157111/> added CHANGELOG.md, modified manifest versions
   We are now using cargo-smart-release to organize releases among with
   conventional commits;

### Bug Fixes

 - <csr-id-8d078c2aadc8b04f7c128a4fb7f8bdb1349935b6/> fixed rustfmt error preventing crate being compiled

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 14 commits contributed to the release over the course of 942 calendar days.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release steam-language-gen-derive v0.1.2, steam-protobuf v0.1.2, steam-language-gen v0.1.2, steam-totp v0.2.2, steamid-parser v0.2.1, steam-mobile v0.3.0 ([`cf773b0`](https://github.com/saskenuba/SteamHelper-rs/commit/cf773b07e0ae68376bf960d12f94ecb96afa9211))
    - Added CHANGELOG.md, modified manifest versions ([`fb87360`](https://github.com/saskenuba/SteamHelper-rs/commit/fb87360214c2f6d1319f467b82b27706ae157111))
    - (steam-protobuf): added license, repo and description to manifest ([`b593dc5`](https://github.com/saskenuba/SteamHelper-rs/commit/b593dc5b687757a74177fb1983289bfd5c94f439))
    - Fixed rustfmt error preventing crate being compiled ([`8d078c2`](https://github.com/saskenuba/SteamHelper-rs/commit/8d078c2aadc8b04f7c128a4fb7f8bdb1349935b6))
    - Rebuild (steam-protobuf): protobufs regenerated with serde support ([`094dbdc`](https://github.com/saskenuba/SteamHelper-rs/commit/094dbdccc8dc5a4ba9cc9c29221b99da3124edf8))
    - Bump, fix (steam-client, steam-protobuf): multiple dep bump and fix ([`d11a7bb`](https://github.com/saskenuba/SteamHelper-rs/commit/d11a7bb751b2314266ebeb3d133381711b446cc2))
    - Bump and regen(steam-probobuf): protobufs are regenerated, + ([`51c9023`](https://github.com/saskenuba/SteamHelper-rs/commit/51c9023b32118a9761e0d6bd3c85a5d007cd4c1a))
    - Submodules ([`690b0d1`](https://github.com/saskenuba/SteamHelper-rs/commit/690b0d1df9400aa7e23cd613046c6f88f93cb7a9))
    - Regenerated protobufs with v3, renamed submodule ([`c15004c`](https://github.com/saskenuba/SteamHelper-rs/commit/c15004c1e829ea931c0fcd620fc2e53090bdb138))
    - Updated submodules ([`c710c11`](https://github.com/saskenuba/SteamHelper-rs/commit/c710c11b80a13bdd2038f481b40a959c9b07d159))
    - Fixed macro to generate protobufs correctly, see log ([`91672c2`](https://github.com/saskenuba/SteamHelper-rs/commit/91672c2c916b9a0be6c188b4330cf27cbd473cbe))
    - Moved protobufs generation outside of build, to main.rs ([`6ceca6a`](https://github.com/saskenuba/SteamHelper-rs/commit/6ceca6a43191d12c0a5f4b84bd12364f292993f4))
    - Added protobuf generation macro ([`85e3b73`](https://github.com/saskenuba/SteamHelper-rs/commit/85e3b73247903b799f78c1ed67e74c3cab88cf6a))
    - Protobuf crate, and automaticaly updated steam protobufs module ([`ff26873`](https://github.com/saskenuba/SteamHelper-rs/commit/ff26873a6df322fe20d5117ef442e64ea264eecc))
</details>

