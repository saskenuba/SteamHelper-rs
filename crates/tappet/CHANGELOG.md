# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### New Features (BREAKING)

 - <csr-id-8490b470ab165e064bf7f6cced4031cb4437b082/> made time_created and time_updated public on tradeoffer response endpoint
 - Made time_created and time_updated public on tradeoffer response endpoint ([`8490b47`](https://github.com/saskenuba/SteamHelper-rs/commit/8490b470ab165e064bf7f6cced4031cb4437b082))
 - Merge pull request #14 from Bash-09/master ([`141ed8f`](https://github.com/saskenuba/SteamHelper-rs/commit/141ed8fc443530ab3fee46399a3c16081a148d10))
 - Some reponses don't have personastateflags ([`3b1e462`](https://github.com/saskenuba/SteamHelper-rs/commit/3b1e462b9a22afcd2acc26edb7ddac1cc35e2f86))
 - Add profilehash to PlayerSummary ([`f44cccb`](https://github.com/saskenuba/SteamHelper-rs/commit/f44cccbc5cf85fc28e5698b11ac4cc4a3912b057))
 - FriendList can be None ([`ccf2e00`](https://github.com/saskenuba/SteamHelper-rs/commit/ccf2e00442cdbe5fdd9085816d1c1a1ff1c932f3))
 - Add GetFriendList response ([`fdb5314`](https://github.com/saskenuba/SteamHelper-rs/commit/fdb53148264f7f7b5461299f51611d39b38f74b3))

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 7 commits contributed to the release over the course of 213 calendar days.
 - 607 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary></details>

## v0.5.0 (2022-06-10)

<csr-id-b2186a6426e52516cecfbda6201028c63229330f/>
<csr-id-14404f4fd83c4c74893e3888693398d98bc3f199/>
<csr-id-a690cf419f36bb737250f0de65aed897e7b5237f/>
<csr-id-02d3b260b9b77a9979073dd3ce2dba792f831ced/>
<csr-id-4bae88942672f2196733df6bc58a82c5a2a7bdf0/>

### Documentation

 - <csr-id-fb87360214c2f6d1319f467b82b27706ae157111/> added CHANGELOG.md, modified manifest versions
   We are now using cargo-smart-release to organize releases among with
   conventional commits;
 - <csr-id-fb999c76e06ac25b09708b0b744115d29520c04f/> added CHANGELOG.md to tappet

### Other

 - <csr-id-b2186a6426e52516cecfbda6201028c63229330f/> tappet minor bump

 - <csr-id-14404f4fd83c4c74893e3888693398d98bc3f199/> updated README with badges and minor fixes

 - <csr-id-a690cf419f36bb737250f0de65aed897e7b5237f/> minor bump

 - <csr-id-02d3b260b9b77a9979073dd3ce2dba792f831ced/> marked optionals types as such, player summaries


### Chore (BREAKING)

 - <csr-id-4bae88942672f2196733df6bc58a82c5a2a7bdf0/> removed the now deprecated endpoints CancelTradeOffer and ..
   DeclineTradeOffer;

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 11 commits contributed to the release over the course of 209 calendar days.
 - 7 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release tappet v0.5.0 ([`019e832`](https://github.com/saskenuba/SteamHelper-rs/commit/019e832c008efd5674cfa35a0659ab145142d674))
    - Added CHANGELOG.md to tappet ([`fb999c7`](https://github.com/saskenuba/SteamHelper-rs/commit/fb999c76e06ac25b09708b0b744115d29520c04f))
    - Tappet minor bump ([`b2186a6`](https://github.com/saskenuba/SteamHelper-rs/commit/b2186a6426e52516cecfbda6201028c63229330f))
    - Removed the now deprecated endpoints CancelTradeOffer and .. ([`4bae889`](https://github.com/saskenuba/SteamHelper-rs/commit/4bae88942672f2196733df6bc58a82c5a2a7bdf0))
    - Added CHANGELOG.md, modified manifest versions ([`fb87360`](https://github.com/saskenuba/SteamHelper-rs/commit/fb87360214c2f6d1319f467b82b27706ae157111))
    - Updated README with badges and minor fixes ([`14404f4`](https://github.com/saskenuba/SteamHelper-rs/commit/14404f4fd83c4c74893e3888693398d98bc3f199))
    - Minor bump ([`a690cf4`](https://github.com/saskenuba/SteamHelper-rs/commit/a690cf419f36bb737250f0de65aed897e7b5237f))
    - Marked optionals types as such, player summaries ([`02d3b26`](https://github.com/saskenuba/SteamHelper-rs/commit/02d3b260b9b77a9979073dd3ce2dba792f831ced))
    - (tappet): fixed and improved readme ([`a882cec`](https://github.com/saskenuba/SteamHelper-rs/commit/a882cec1c7c98f202deca3518ae42c3fd21ad343))
    - (tappet): missing imports for "blocking" feature ([`25b0114`](https://github.com/saskenuba/SteamHelper-rs/commit/25b011474873280e088e8dbc3da21a3bbe8ce73c))
    - (web-api, web-api-derive) renamed "steam-web-api" to "tappet" ([`4e59ded`](https://github.com/saskenuba/SteamHelper-rs/commit/4e59ded6883fb7201ae8554d747b2ddb2c057dce))
</details>

