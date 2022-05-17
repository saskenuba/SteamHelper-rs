# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.0 (2022-05-17)

<csr-id-0fc7ca6876a61d07945a4f6d5a0a937a44fe6af2/>
<csr-id-14404f4fd83c4c74893e3888693398d98bc3f199/>
<csr-id-5be4545d48846cf7e6ba166a545ce77fd451b26a/>
<csr-id-23f13a9e8927375f8a5dcd5be005e1c878132157/>

### New Features (BREAKING)

 - <csr-id-fdcf4076fe266964f5e8c9aa5beb81ab38281a51/> Added accept/deny mobile confirmations on CLI.
   * You can now accept and deny trade requests through the CLI;
* Bumped clap, uuid and dialoguer deps to latest versions;
* Refactored code of CLI to be more readable;

### Refactor

 - <csr-id-0fc7ca6876a61d07945a4f6d5a0a937a44fe6af2/> decoupled disk logic into fn `read_from_disk` on utils, +
   Added more methods to create a maFile

### Documentation

 - <csr-id-60e3691a305ec8cd3f32fdf5ed68f6b28185b42d/> added CHANGELOG.md, prepare smart-release
 - <csr-id-fb87360214c2f6d1319f467b82b27706ae157111/> added CHANGELOG.md, modified manifest versions
   We are now using cargo-smart-release to organize releases among with
   conventional commits;

### Other

 - <csr-id-14404f4fd83c4c74893e3888693398d98bc3f199/> updated README with badges and minor fixes

 - <csr-id-5be4545d48846cf7e6ba166a545ce77fd451b26a/> readme changes

 - <csr-id-23f13a9e8927375f8a5dcd5be005e1c878132157/> add convenience fn `has_trade_offer_id` to Confirmations


### Commit Statistics

<csr-read-only-do-not-edit/>

 - 8 commits contributed to the release over the course of 186 calendar days.
 - 6 commits where understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' where seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - added CHANGELOG.md, modified manifest versions ([`fb87360`](https://github.comgit//saskenuba/SteamHelper-rs/commit/fb87360214c2f6d1319f467b82b27706ae157111))
    - Added accept/deny mobile confirmations on CLI. ([`fdcf407`](https://github.comgit//saskenuba/SteamHelper-rs/commit/fdcf4076fe266964f5e8c9aa5beb81ab38281a51))
    - decoupled disk logic into fn `read_from_disk` on utils, + ([`0fc7ca6`](https://github.comgit//saskenuba/SteamHelper-rs/commit/0fc7ca6876a61d07945a4f6d5a0a937a44fe6af2))
    - updated README with badges and minor fixes ([`14404f4`](https://github.comgit//saskenuba/SteamHelper-rs/commit/14404f4fd83c4c74893e3888693398d98bc3f199))
    - readme changes ([`5be4545`](https://github.comgit//saskenuba/SteamHelper-rs/commit/5be4545d48846cf7e6ba166a545ce77fd451b26a))
    - add convenience fn `has_trade_offer_id` to Confirmations ([`23f13a9`](https://github.comgit//saskenuba/SteamHelper-rs/commit/23f13a9e8927375f8a5dcd5be005e1c878132157))
    - (mobile, trading): fixes to manifest ([`43c3984`](https://github.comgit//saskenuba/SteamHelper-rs/commit/43c3984bf594bf6eb3d82c7c955e0b35d8db3d48))
    - renamed from steam-auth to steam-mobile because of crates.io ([`749e6fc`](https://github.comgit//saskenuba/SteamHelper-rs/commit/749e6fc8c36af282ba18492e0b9f9f53ec7d00ed))
</details>

