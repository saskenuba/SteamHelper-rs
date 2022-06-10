# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.5.0 (2022-06-10)

### Chore

 - <csr-id-df44ea2e40d951feca1a01490c9054ae94be7098/> cargo.toml dep on tappet


### Documentation

 - <csr-id-fb87360214c2f6d1319f467b82b27706ae157111/> added CHANGELOG.md, modified manifest versions
   We are now using cargo-smart-release to organize releases among with
   conventional commits;

### New Features

 - <csr-id-0ea163967c2009e1775d14b39704d497bf001e03/> 0.0.1 new crate to handle trade operations

### Other

 - <csr-id-14404f4fd83c4c74893e3888693398d98bc3f199/> updated README with badges and minor fixes

 - <csr-id-2335baf050b1a30a3adcfbe9363e7451bee86f7a/> reformatted modules with new rustfmt setting

 - <csr-id-0a349d0bef0e59376961bcf9da0d6a165be78b71/> added Tradelink type to validate tradelinks


### Refactor

 - <csr-id-e5aa817486fdaa5b989a2d002cfbd21fb706f4a3/> changed some methods to TradeOffer
 - <csr-id-0bb0e68b73c86347e16e65b223121737732ee75d/> +
   - Transferred some constants that were on steam-auth to steam-trading

### Chore (BREAKING)

 - <csr-id-89389a5788ba7a7bb26b48cc2ad7b171e9386185/> fix API endpoint deprecation for DeclineTradeOffer ..
   and CancelTradeOffer;
   
   Now, the only way to cancel and decline an offer is through the client;
   * Fixed some clippy lints

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 25 commits contributed to the release over the course of 707 calendar days.
 - 9 commits where understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' where seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - cargo.toml dep on tappet ([`df44ea2`](https://github.comgit//saskenuba/SteamHelper-rs/commit/df44ea2e40d951feca1a01490c9054ae94be7098))
    - fix API endpoint deprecation for DeclineTradeOffer .. ([`89389a5`](https://github.comgit//saskenuba/SteamHelper-rs/commit/89389a5788ba7a7bb26b48cc2ad7b171e9386185))
    - added CHANGELOG.md, modified manifest versions ([`fb87360`](https://github.comgit//saskenuba/SteamHelper-rs/commit/fb87360214c2f6d1319f467b82b27706ae157111))
    - updated README with badges and minor fixes ([`14404f4`](https://github.comgit//saskenuba/SteamHelper-rs/commit/14404f4fd83c4c74893e3888693398d98bc3f199))
    - (mobile, trading): fixes to manifest ([`43c3984`](https://github.comgit//saskenuba/SteamHelper-rs/commit/43c3984bf594bf6eb3d82c7c955e0b35d8db3d48))
    - (trading): set missing versions of tappet and steam-mobile ([`62c475d`](https://github.comgit//saskenuba/SteamHelper-rs/commit/62c475daa0ef7d6f147b174ac38bf939d7b61f63))
    - (trading): set version of helpers crates, added steam_guard check ([`8bf86d1`](https://github.comgit//saskenuba/SteamHelper-rs/commit/8bf86d1aa86315bd3efe2c364d8346a0ca3dcabc))
    - (trading, client): fixed tappet rename ([`ce6172a`](https://github.comgit//saskenuba/SteamHelper-rs/commit/ce6172aa2c57d917d5345240a51c4e51a4b34fff))
    - (trading): fixed steam-mobile dep rename ([`5d71b39`](https://github.comgit//saskenuba/SteamHelper-rs/commit/5d71b395736526fe99b686a1721e86d3d7a01b8b))
    - (trading): fixed typo ([`09ce006`](https://github.comgit//saskenuba/SteamHelper-rs/commit/09ce0063451cf24b2c5bbc424b3f9b25cb664221))
    - (trading): bumped to 0.4.2 since some changes will break code ([`2a9a389`](https://github.comgit//saskenuba/SteamHelper-rs/commit/2a9a389f1cbab67d5240151d6fde1c52391a937d))
    - (trading): separated trade validation errors from "trading" errors ([`398593c`](https://github.comgit//saskenuba/SteamHelper-rs/commit/398593c943b5b9115e564af7948a382851de5723))
    - (trading): bump to version 0.3.2, + ([`47dd5a0`](https://github.comgit//saskenuba/SteamHelper-rs/commit/47dd5a05448e4c8858ca59bdb623f97c2c966723))
    - (trading): better error message to unknown error ([`bbf51c2`](https://github.comgit//saskenuba/SteamHelper-rs/commit/bbf51c211e351d28e169f942b852929fa75a94cf))
    - (trading) added tests, tokio dep removed, lib executor agnostic ([`c55b08a`](https://github.comgit//saskenuba/SteamHelper-rs/commit/c55b08a41afde923ae58c7af34e30f6aa49166a6))
    - reformatted modules with new rustfmt setting ([`2335baf`](https://github.comgit//saskenuba/SteamHelper-rs/commit/2335baf050b1a30a3adcfbe9363e7451bee86f7a))
    - refactor and bump(steam-trading): 0.3.0 breaking changes ([`9301610`](https://github.comgit//saskenuba/SteamHelper-rs/commit/9301610216946f04189d40f1eed4e765cf85208d))
    - added Tradelink type to validate tradelinks ([`0a349d0`](https://github.comgit//saskenuba/SteamHelper-rs/commit/0a349d0bef0e59376961bcf9da0d6a165be78b71))
    - features and bump(steam-web-api): trading feature gate, fixes ([`00c1854`](https://github.comgit//saskenuba/SteamHelper-rs/commit/00c185465f480292880fa0ad794f467f0e16915a))
    - minor bump to 0.2.0(steam-trading): features and refactor ([`818682b`](https://github.comgit//saskenuba/SteamHelper-rs/commit/818682bc9c2a60ea28211ec0a322c212b5a2a509))
    - feature (steam-trading): Trade Offer accepting is now working ([`f1ab69b`](https://github.comgit//saskenuba/SteamHelper-rs/commit/f1ab69bf436c997b7ab0a1d1c7696063ff1f0408))
    - changed some methods to TradeOffer ([`e5aa817`](https://github.comgit//saskenuba/SteamHelper-rs/commit/e5aa817486fdaa5b989a2d002cfbd21fb706f4a3))
    - features and refactor(steam-trading): check log + ([`a039ac3`](https://github.comgit//saskenuba/SteamHelper-rs/commit/a039ac360714deae6cd4f44b5a82ee327ad08330))
    - + ([`0bb0e68`](https://github.comgit//saskenuba/SteamHelper-rs/commit/0bb0e68b73c86347e16e65b223121737732ee75d))
    - 0.0.1 new crate to handle trade operations ([`0ea1639`](https://github.comgit//saskenuba/SteamHelper-rs/commit/0ea163967c2009e1775d14b39704d497bf001e03))
</details>

