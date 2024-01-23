# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.4.0 (2024-01-23)

<csr-id-154212b00831bebc6ff4b7c351fee7c78dc90aa2/>
<csr-id-f37078a6c55650561b60faa6e9daac0b04efad41/>
<csr-id-179492088b413f18b5a6ea83167a2d5c807f58cc/>
<csr-id-6457ec23fabec9fe7965b93eabea3ca4b850fe33/>
<csr-id-54af8c01c71bbc511eae6d37e537b465842fe226/>
<csr-id-af9b9350dcefdf5e74e71fa890a365ac508571c4/>

### Documentation

 - <csr-id-0142d141bebfbe9d641ef98098ee9f9c8acd1757/> fixed wrong docs

### New Features

<csr-id-9d864866b00e05bcf1cb1b7db389b5a2a4c11557/>
<csr-id-65121ed55a3eb6d6b3068cc7d6ffe0bf6dc74c06/>
<csr-id-42998bd67bd0fc0bdae73db9b191ebc3461fb551/>
<csr-id-8cf954b4d03e29891ea37f219c086b025faab05d/>
<csr-id-95568ed52550c821212862ad0515e3cf2a69b6f4/>
<csr-id-897479ea62281f23a222bf735ceda6a22c557046/>
<csr-id-02f302c9e67aebab3a7c9a892100b12fd5537de0/>
<csr-id-fb218a3bdb7a047050c307b47c29800e51c59608/>
<csr-id-cad5810fed6cc3298a9497cb367ca7ebbb113d96/>
<csr-id-d88d9c3cd338c670db487bda8482ebb33ddb76b6/>

 - <csr-id-fe6c19345b15cb4b63f75b05418e0213e8b2b665/> cli is compiling again
 - <csr-id-8fea774279551db86f6b92315a059ab5339ced9b/> adjusted confirmation inner methods
 - <csr-id-c3b7298bf8593a494e51078bb7b4b31582783835/> registration of new api key is now working again; sending confirmations is working again
 - <csr-id-485daa67302716dbdd7c3de61722db4adda34727/> fetch of login RSA key is now protobuf backed
 - <csr-id-db6e83a347b6aa3df4f0e1811cd354861839934d/> User is now Steam<T>, where T is the user having a mafile or not.
   this allows methods that requires a mafile to not show up on completions
   
   * SteamUser states are SteamUser<PresentMaFile> and SteamUser<AbsentMaFile>;
* Confirmation retrieval is now working correctly;
* API Key registration is almost working again, only sending confirmations are
   needed;
* Removed manually adding cookies after pinging steam domains, this is done
   automatically now by request method;

### Bug Fixes

 - <csr-id-d7799dd1f1eea85e0af06f50b4b791bfe0697b9c/> less debug! calls on proto request
 - <csr-id-996e28a7930f76adc8dd95ff90052557540e027d/> fixed STEAM_COMMUNITY_HOST
 - <csr-id-6505161011e05dad07450761006aae803858458d/> fixed an issue where request cookies weren't being passed correctly
 - <csr-id-7bdf9dff27171abb268cb7982c606f76c42b0267/> cleanup inner requests with new request_and_decode functions
 - <csr-id-d4323529730939b9dffa5af1aa0768591d414577/> clippy lints, renamed dump_cookies_by_name -> dump_cookies_by_domain_and_name

### Other

 - <csr-id-154212b00831bebc6ff4b7c351fee7c78dc90aa2/> removed num-derive, num-traits and added serde_repr from workspace
 - <csr-id-f37078a6c55650561b60faa6e9daac0b04efad41/> added some common used deps
 - <csr-id-179492088b413f18b5a6ea83167a2d5c807f58cc/> fixed clippy warnings
 - <csr-id-6457ec23fabec9fe7965b93eabea3ca4b850fe33/> added InternalError::GeneralFailure; set_steamid returns Result<T, InternalError>
 - <csr-id-54af8c01c71bbc511eae6d37e537b465842fe226/> typos and clippy lints on app cli
 - <csr-id-af9b9350dcefdf5e74e71fa890a365ac508571c4/> moved shared dependencies into workspace, added them to steam-mobile

### Other

 - <csr-id-dd51f9fda6bcdaccb3f6baeff70bf03fd325e3c4/> added changelog and bumped to 0.4.0

### New Features (BREAKING)

<csr-id-757ff98ce1b619715ea076b4241e3252156e0757/>

 - <csr-id-d043dd67293431c9a81ee7c5f5b4e02e955c32e8/> Auth is now typed with SteamAuthenticator<Authenticated> with proper methods;
   * access_token is now working correctly for QueryStatus, for example, but still
   need to adapt all linker related methods;
* Removed weird wrapper around Confirmations;
* Only method available on SteamAuthenticator<Unauthenticated> is login mostly;
* BREAKING CHANGE: Added InternalError to return types;
* BREAKING CHANGE: renamed MobileAuthFile from_str -> from_json;
* Additional request and deserialize functions to avoid duplication;
* CachedInfo is now SteamCache

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 35 commits contributed to the release over the course of 18 calendar days.
 - 580 days passed between releases.
 - 30 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Added changelog and bumped to 0.4.0 ([`dd51f9f`](https://github.com/saskenuba/SteamHelper-rs/commit/dd51f9fda6bcdaccb3f6baeff70bf03fd325e3c4))
    - Merge pull request #17 from saskenuba/mobile-confirmations-fix ([`bbec693`](https://github.com/saskenuba/SteamHelper-rs/commit/bbec69346043c586fcd3d8cdf2b04cc732f0b0d5))
    - Cli is compiling again ([`fe6c193`](https://github.com/saskenuba/SteamHelper-rs/commit/fe6c19345b15cb4b63f75b05418e0213e8b2b665))
    - Adjusted confirmation inner methods ([`8fea774`](https://github.com/saskenuba/SteamHelper-rs/commit/8fea774279551db86f6b92315a059ab5339ced9b))
    - Registration of new api key is now working again; sending confirmations is working again ([`c3b7298`](https://github.com/saskenuba/SteamHelper-rs/commit/c3b7298bf8593a494e51078bb7b4b31582783835))
    - Less debug! calls on proto request ([`d7799dd`](https://github.com/saskenuba/SteamHelper-rs/commit/d7799dd1f1eea85e0af06f50b4b791bfe0697b9c))
    - Fixed STEAM_COMMUNITY_HOST ([`996e28a`](https://github.com/saskenuba/SteamHelper-rs/commit/996e28a7930f76adc8dd95ff90052557540e027d))
    - Fetch of login RSA key is now protobuf backed ([`485daa6`](https://github.com/saskenuba/SteamHelper-rs/commit/485daa67302716dbdd7c3de61722db4adda34727))
    - User is now Steam<T>, where T is the user having a mafile or not. ([`db6e83a`](https://github.com/saskenuba/SteamHelper-rs/commit/db6e83a347b6aa3df4f0e1811cd354861839934d))
    - Cookies are handled directly by request, no need to manually input them on storage ([`9d86486`](https://github.com/saskenuba/SteamHelper-rs/commit/9d864866b00e05bcf1cb1b7db389b5a2a4c11557))
    - Removed num-derive, num-traits and added serde_repr from workspace ([`154212b`](https://github.com/saskenuba/SteamHelper-rs/commit/154212b00831bebc6ff4b7c351fee7c78dc90aa2))
    - Removed unused confirmations scrapers, confirmations are now a json response ([`65121ed`](https://github.com/saskenuba/SteamHelper-rs/commit/65121ed55a3eb6d6b3068cc7d6ffe0bf6dc74c06))
    - Removed unused types, added new confirmations kinds, adjusted Confirmation ([`42998bd`](https://github.com/saskenuba/SteamHelper-rs/commit/42998bd67bd0fc0bdae73db9b191ebc3461fb551))
    - Feat!(mobile): now all login is done through valid prototobufs ([`73862f1`](https://github.com/saskenuba/SteamHelper-rs/commit/73862f108903225537ab2c3a7eb489caeab2ad13))
    - Added some common used deps ([`f37078a`](https://github.com/saskenuba/SteamHelper-rs/commit/f37078a6c55650561b60faa6e9daac0b04efad41))
    - Merge pull request #16 from saskenuba/typed-mobile-refactor ([`4048602`](https://github.com/saskenuba/SteamHelper-rs/commit/4048602966d0a9981edc61253c7c74ba370c81b7))
    - Auth is now typed with SteamAuthenticator<Authenticated> with proper methods; ([`d043dd6`](https://github.com/saskenuba/SteamHelper-rs/commit/d043dd67293431c9a81ee7c5f5b4e02e955c32e8))
    - Merge pull request #15 from saskenuba/steam-mobile-login-revamp ([`d6cf2ef`](https://github.com/saskenuba/SteamHelper-rs/commit/d6cf2ef64b3efbd95dbd7d8de738c2a7d956ff2d))
    - Fixed clippy warnings ([`1794920`](https://github.com/saskenuba/SteamHelper-rs/commit/179492088b413f18b5a6ea83167a2d5c807f58cc))
    - Fixed wrong docs ([`0142d14`](https://github.com/saskenuba/SteamHelper-rs/commit/0142d141bebfbe9d641ef98098ee9f9c8acd1757))
    - Fixed an issue where request cookies weren't being passed correctly ([`6505161`](https://github.com/saskenuba/SteamHelper-rs/commit/6505161011e05dad07450761006aae803858458d))
    - Removed warnings, fixed docs ([`8cf954b`](https://github.com/saskenuba/SteamHelper-rs/commit/8cf954b4d03e29891ea37f219c086b025faab05d))
    - Clippy lints, docs, renamed internal functions ([`95568ed`](https://github.com/saskenuba/SteamHelper-rs/commit/95568ed52550c821212862ad0515e3cf2a69b6f4))
    - Login and storage of cookies working correctly, cleannup of docs and comments ([`897479e`](https://github.com/saskenuba/SteamHelper-rs/commit/897479ea62281f23a222bf735ceda6a22c557046))
    - Added InternalError::GeneralFailure; set_steamid returns Result<T, InternalError> ([`6457ec2`](https://github.com/saskenuba/SteamHelper-rs/commit/6457ec23fabec9fe7965b93eabea3ca4b850fe33))
    - Typos and clippy lints on app cli ([`54af8c0`](https://github.com/saskenuba/SteamHelper-rs/commit/54af8c01c71bbc511eae6d37e537b465842fe226))
    - Cleanup inner requests with new request_and_decode functions ([`7bdf9df`](https://github.com/saskenuba/SteamHelper-rs/commit/7bdf9dff27171abb268cb7982c606f76c42b0267))
    - Changes ([`757ff98`](https://github.com/saskenuba/SteamHelper-rs/commit/757ff98ce1b619715ea076b4241e3252156e0757))
    - All improved internal APIs returning only errors from own crate ([`02f302c`](https://github.com/saskenuba/SteamHelper-rs/commit/02f302c9e67aebab3a7c9a892100b12fd5537de0))
    - New steam login method, removed unused old types ([`fb218a3`](https://github.com/saskenuba/SteamHelper-rs/commit/fb218a3bdb7a047050c307b47c29800e51c59608))
    - Clippy lints, renamed dump_cookies_by_name -> dump_cookies_by_domain_and_name ([`d432352`](https://github.com/saskenuba/SteamHelper-rs/commit/d4323529730939b9dffa5af1aa0768591d414577))
    - Feat!(mobile): refactored Confirmations API to return Iterators on methods ([`2ae8d2b`](https://github.com/saskenuba/SteamHelper-rs/commit/2ae8d2bc84165d28b2b62348d006bae4b25d7f97))
    - Moved shared dependencies into workspace, added them to steam-mobile ([`af9b935`](https://github.com/saskenuba/SteamHelper-rs/commit/af9b9350dcefdf5e74e71fa890a365ac508571c4))
    - Added remove_authenticator method, clippy lints and fixes ([`cad5810`](https://github.com/saskenuba/SteamHelper-rs/commit/cad5810fed6cc3298a9497cb367ca7ebbb113d96))
    - Remove Rc<RefCell<T>> from cookie jar and mobile client; organized cargo.toml ([`d88d9c3`](https://github.com/saskenuba/SteamHelper-rs/commit/d88d9c3cd338c670db487bda8482ebb33ddb76b6))
</details>

<csr-unknown>
 cookies are handled directly by request, no need to manually input them on storage removed unused confirmations scrapers, confirmations are now a json response removed unused types, added new confirmations kinds, adjusted Confirmation removed warnings, fixed docs clippy lints, docs, renamed internal functions login and storage of cookies working correctly, cleannup of docs and comments all improved internal APIs returning only errors from own crate new steam login method, removed unused old types added remove_authenticator method, clippy lints and fixes Remove Rc<RefCell<T>> from cookie jar and mobile client; organized cargo.tomlBREAKING CHANGES: changes<csr-unknown/>

## 0.3.1 (2022-06-22)

<csr-id-e266af7ab0b709059f71c63e4e73eeea323fd1d4/>

### Refactor

 - <csr-id-e266af7ab0b709059f71c63e4e73eeea323fd1d4/> improved error handling of login and confirmations fetch


### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 35 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release steam-mobile v0.3.1 ([`0f532de`](https://github.com/saskenuba/SteamHelper-rs/commit/0f532dec00d961de6122295769aea6a5e9accd70))
    - Improved error handling of login and confirmations fetch ([`e266af7`](https://github.com/saskenuba/SteamHelper-rs/commit/e266af7ab0b709059f71c63e4e73eeea323fd1d4))
</details>

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

 - 11 commits contributed to the release over the course of 186 calendar days.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release steamid-parser v0.2.1, steam-mobile v0.3.0 ([`fd04253`](https://github.com/saskenuba/SteamHelper-rs/commit/fd0425344eb5d24093154320cc0ed81bf82a0b1a))
    - Release steamid-parser v0.2.1, steam-mobile v0.3.0 ([`ea7632d`](https://github.com/saskenuba/SteamHelper-rs/commit/ea7632d2fe5fcd85b48315f246f815afba88e62e))
    - Release steam-language-gen-derive v0.1.2, steam-protobuf v0.1.2, steam-language-gen v0.1.2, steam-totp v0.2.2, steamid-parser v0.2.1, steam-mobile v0.3.0 ([`cf773b0`](https://github.com/saskenuba/SteamHelper-rs/commit/cf773b07e0ae68376bf960d12f94ecb96afa9211))
    - Added CHANGELOG.md, modified manifest versions ([`fb87360`](https://github.com/saskenuba/SteamHelper-rs/commit/fb87360214c2f6d1319f467b82b27706ae157111))
    - Added accept/deny mobile confirmations on CLI. ([`fdcf407`](https://github.com/saskenuba/SteamHelper-rs/commit/fdcf4076fe266964f5e8c9aa5beb81ab38281a51))
    - Decoupled disk logic into fn `read_from_disk` on utils, + ([`0fc7ca6`](https://github.com/saskenuba/SteamHelper-rs/commit/0fc7ca6876a61d07945a4f6d5a0a937a44fe6af2))
    - Updated README with badges and minor fixes ([`14404f4`](https://github.com/saskenuba/SteamHelper-rs/commit/14404f4fd83c4c74893e3888693398d98bc3f199))
    - Readme changes ([`5be4545`](https://github.com/saskenuba/SteamHelper-rs/commit/5be4545d48846cf7e6ba166a545ce77fd451b26a))
    - Add convenience fn `has_trade_offer_id` to Confirmations ([`23f13a9`](https://github.com/saskenuba/SteamHelper-rs/commit/23f13a9e8927375f8a5dcd5be005e1c878132157))
    - (mobile, trading): fixes to manifest ([`43c3984`](https://github.com/saskenuba/SteamHelper-rs/commit/43c3984bf594bf6eb3d82c7c955e0b35d8db3d48))
    - Renamed from steam-auth to steam-mobile because of crates.io ([`749e6fc`](https://github.com/saskenuba/SteamHelper-rs/commit/749e6fc8c36af282ba18492e0b9f9f53ec7d00ed))
</details>

