# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.2 (2022-05-17)

<csr-id-843013c43386a837de6b816f65ab2e520677bab4/>
<csr-id-7e079927b99f2078f455fa1d85be28465846e9b7/>

### Refactor

 - <csr-id-843013c43386a837de6b816f65ab2e520677bab4/> renamed traits, impl serializablebytes for proto..
   dropped downcast-rs dep

### Documentation

 - <csr-id-60e3691a305ec8cd3f32fdf5ed68f6b28185b42d/> added CHANGELOG.md, prepare smart-release
 - <csr-id-fb87360214c2f6d1319f467b82b27706ae157111/> added CHANGELOG.md, modified manifest versions
   We are now using cargo-smart-release to organize releases among with
   conventional commits;

### Other

 - <csr-id-7e079927b99f2078f455fa1d85be28465846e9b7/> add licenses to crates,
   * Removed unused deps on steam-language-gen;
   
   Preparing for release on crates.io

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 9 commits contributed to the release over the course of 898 calendar days.
 - 3 commits where understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' where seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - added CHANGELOG.md, modified manifest versions ([`fb87360`](https://github.comgit//saskenuba/SteamHelper-rs/commit/fb87360214c2f6d1319f467b82b27706ae157111))
    - (steam-lang-derive): bump to 0.1.1 ([`05152ce`](https://github.comgit//saskenuba/SteamHelper-rs/commit/05152ce6395cf5d99647a62b3becdac60467e89a))
    - renamed traits, impl serializablebytes for proto.. ([`843013c`](https://github.comgit//saskenuba/SteamHelper-rs/commit/843013c43386a837de6b816f65ab2e520677bab4))
    - fix (steam-lang-gen): message structs are now packed, and copy types ([`10aa5f8`](https://github.comgit//saskenuba/SteamHelper-rs/commit/10aa5f8cffe944cb9ec82ea392db0e3bf715ff62))
    - add licenses to crates, ([`7e07992`](https://github.comgit//saskenuba/SteamHelper-rs/commit/7e079927b99f2078f455fa1d85be28465846e9b7))
    - big refactor and important bug fix. check log for details + ([`9fc8a4e`](https://github.comgit//saskenuba/SteamHelper-rs/commit/9fc8a4e2686ffc6d5cff86822f07a73d2c8f12fa))
    - parser now emit messages with custom attr macro containing emsg, + ([`101b688`](https://github.comgit//saskenuba/SteamHelper-rs/commit/101b688cb8b9a0eb5105ccfc9d465b8c3951a9eb))
    - huge refactor, check log ([`5369d20`](https://github.comgit//saskenuba/SteamHelper-rs/commit/5369d20d9f28cde94b96976dab5e2909f30ddb3f))
    - added custom derive for steam messages (to de/serialize) w/ bincode ([`2d19e42`](https://github.comgit//saskenuba/SteamHelper-rs/commit/2d19e42ca70276199019292cf040d36ee620ff56))
</details>

