# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.3 (2026-10-18)

### Chore

 - <csr-id-8cba64f4e70cd363e2fb272f9fe16df5bb95c147/> remove unused dependencies

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 1 commit contributed to the release over the course of 1 calendar day.
 - 1615 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Remove unused dependencies ([`8cba64f`](https://github.com/saskenuba/SteamHelper-rs/commit/8cba64f4e70cd363e2fb272f9fe16df5bb95c147))
</details>

## 0.2.2 (2022-05-17)

### Documentation

 - <csr-id-fb87360214c2f6d1319f467b82b27706ae157111/> added CHANGELOG.md, modified manifest versions
   We are now using cargo-smart-release to organize releases among with
   conventional commits;

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 26 commits contributed to the release over the course of 943 calendar days.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release steam-language-gen-derive v0.1.2, steam-protobuf v0.1.2, steam-language-gen v0.1.2, steam-totp v0.2.2, steamid-parser v0.2.1, steam-mobile v0.3.0 ([`cf773b0`](https://github.com/saskenuba/SteamHelper-rs/commit/cf773b07e0ae68376bf960d12f94ecb96afa9211))
    - Added CHANGELOG.md, modified manifest versions ([`fb87360`](https://github.com/saskenuba/SteamHelper-rs/commit/fb87360214c2f6d1319f467b82b27706ae157111))
    - (steam-totp): added repo, license and description to manifest ([`7aa6f58`](https://github.com/saskenuba/SteamHelper-rs/commit/7aa6f58d9aaea48a292ea4fefc9909ea97350e7a))
    - (totp): reqwest dep bump, rustfmt, bumped to 0.2.1 ([`144b67f`](https://github.com/saskenuba/SteamHelper-rs/commit/144b67fad9cdbb4b973a79dcfb62073e7a63ac3d))
    - Fix, bump: serde internal formatter auto-import by ide, reqwest bump ([`cf962bc`](https://github.com/saskenuba/SteamHelper-rs/commit/cf962bc01538e3a989227cd7436b158e2b2f8a99))
    - Refactor and features: Add authenticator is now working! ([`a6b1196`](https://github.com/saskenuba/SteamHelper-rs/commit/a6b1196faff9aebad61396ef313d82e1afc78d71))
    - Add more error types, moved utils, ... + ([`1d0d732`](https://github.com/saskenuba/SteamHelper-rs/commit/1d0d73215a5b6bcf184ad473fea34b62c39b49ae))
    - Fixed overflow when subtracting less than zero offset, deps bump ([`bb123c4`](https://github.com/saskenuba/SteamHelper-rs/commit/bb123c41269ef232b283bc6bc224398bf6dfab70))
    - Merge branch 'master' of github.com:saskenuba/SteamHelper-rs ([`95563c1`](https://github.com/saskenuba/SteamHelper-rs/commit/95563c1a343a1ca5acae1ee599fa5aad7920d8f2))
    - Merge pull request #5 from Leorii/2-port-steam-totp ([`5cd7068`](https://github.com/saskenuba/SteamHelper-rs/commit/5cd7068abd3389abea3a12551c29035aac64300c))
    - Add documentation ([`4a8385b`](https://github.com/saskenuba/SteamHelper-rs/commit/4a8385bd60852a3750261a2b7929fc044b0fcf3b))
    - Implement generate_auth_code_async ([`cd28af4`](https://github.com/saskenuba/SteamHelper-rs/commit/cd28af4479f8f6461ef5b23d33cbe3d2d4d9667e))
    - Add methods for working with time offset from Steam servers ([`86dd482`](https://github.com/saskenuba/SteamHelper-rs/commit/86dd4822177af5bf373b0f219e7e5afea9fc16ea))
    - Skeleton next methods ([`833e924`](https://github.com/saskenuba/SteamHelper-rs/commit/833e924302245e11af01731b74d78b97bd73c9f9))
    - Implement get_device_id ([`6d67c59`](https://github.com/saskenuba/SteamHelper-rs/commit/6d67c5916f2fbfdbd0df4daa9900e14880d93942))
    - Add generate_confirmation_key and big refactor ([`95dd97b`](https://github.com/saskenuba/SteamHelper-rs/commit/95dd97b01b44236a8385b7a9f5a3acf68f8c5af4))
    - Restructure code and make Time struct public ([`161dcc7`](https://github.com/saskenuba/SteamHelper-rs/commit/161dcc70c9b26e6db9fa03a1d5a50c9de9feb896))
    - Merge pull request #4 from Leorii/2-port-steam-totp ([`36a34a1`](https://github.com/saskenuba/SteamHelper-rs/commit/36a34a129b0c01a3ec10a6e8745423ab43029fae))
    - Update documentation ([`c824b69`](https://github.com/saskenuba/SteamHelper-rs/commit/c824b69865c38d00ad6863c8d27e3b52231d2a29))
    - Clean up code ([`770350c`](https://github.com/saskenuba/SteamHelper-rs/commit/770350cff6b1a09714b5e1096b1877d6d9a95c3d))
    - Just return decode errors directly without wrapping ([`8117b51`](https://github.com/saskenuba/SteamHelper-rs/commit/8117b51279b7c83e0804b22a0beac51bf6564ba3))
    - Update generate_auth_code to work with hex and b64 ([`794e4ed`](https://github.com/saskenuba/SteamHelper-rs/commit/794e4ed164c6b47c027743c9b69ab952275c414d))
    - Update Error impl to remove deprecated methods ([`2cdfc8b`](https://github.com/saskenuba/SteamHelper-rs/commit/2cdfc8bc78577ef9118c19d11bfc8bd2a79a7b40))
    - Better error handling ([`b59aed3`](https://github.com/saskenuba/SteamHelper-rs/commit/b59aed30a28571ad9b2345379064071e9e503a10))
    - Implement generate_auth_code for steam-totp ([`955a0fe`](https://github.com/saskenuba/SteamHelper-rs/commit/955a0fed84e7adbbddabc498ee19ff343abeb869))
    - Initial commit ([`748e315`](https://github.com/saskenuba/SteamHelper-rs/commit/748e315135d81993a34647c9a2fc584f5678a35f))
</details>

