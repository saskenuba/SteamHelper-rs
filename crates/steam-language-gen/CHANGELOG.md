# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.2 (2022-05-17)

<csr-id-843013c43386a837de6b816f65ab2e520677bab4/>
<csr-id-d1665f62bc81693f5055cd323ca8f8790ca93c63/>
<csr-id-e3b425dafa7bf75ab287c23b365bf7a151eb2361/>
<csr-id-690b0d1df9400aa7e23cd613046c6f88f93cb7a9/>
<csr-id-7e079927b99f2078f455fa1d85be28465846e9b7/>

### Refactor

 - <csr-id-843013c43386a837de6b816f65ab2e520677bab4/> renamed traits, impl serializablebytes for proto..
   dropped downcast-rs dep
 - <csr-id-d1665f62bc81693f5055cd323ca8f8790ca93c63/> enums now implement copy

 - <csr-id-e3b425dafa7bf75ab287c23b365bf7a151eb2361/> added feature gate for generator deps


### Documentation

 - <csr-id-60e3691a305ec8cd3f32fdf5ed68f6b28185b42d/> added CHANGELOG.md, prepare smart-release


### Other

 - <csr-id-690b0d1df9400aa7e23cd613046c6f88f93cb7a9/> submodules
 - <csr-id-7e079927b99f2078f455fa1d85be28465846e9b7/> add licenses to crates,
   * Removed unused deps on steam-language-gen;
   
   Preparing for release on crates.io

### Bug Fixes

 - <csr-id-8dc2d2c1bafcf7684cae908147038bb634b7c96c/> messages should have pub fields by default

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 31 commits contributed to the release over the course of 921 calendar days.
 - 7 commits where understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' where seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - added CHANGELOG.md, prepare smart-release ([`60e3691`](https://github.comgit//saskenuba/SteamHelper-rs/commit/60e3691a305ec8cd3f32fdf5ed68f6b28185b42d))
    - (steam-lang): bump to 0.1.1 ([`6f274ac`](https://github.comgit//saskenuba/SteamHelper-rs/commit/6f274ac18da476ae9391fd1954745194a3756118))
    - (steam-lang): updated deps to latest on crates.io ([`29fbdb2`](https://github.comgit//saskenuba/SteamHelper-rs/commit/29fbdb21e7bffbbf3c60ae3e7aa15b82c2e7c7ed))
    - minor changes to generator and generate fns ([`b955ecd`](https://github.comgit//saskenuba/SteamHelper-rs/commit/b955ecd9ab6b0f14e855f00ea03018d171203c16))
    - renamed traits, impl serializablebytes for proto.. ([`843013c`](https://github.comgit//saskenuba/SteamHelper-rs/commit/843013c43386a837de6b816f65ab2e520677bab4))
    - messages should have pub fields by default ([`8dc2d2c`](https://github.comgit//saskenuba/SteamHelper-rs/commit/8dc2d2c1bafcf7684cae908147038bb634b7c96c))
    - fix (steam-lang-gen): message structs are now packed, and copy types ([`10aa5f8`](https://github.comgit//saskenuba/SteamHelper-rs/commit/10aa5f8cffe944cb9ec82ea392db0e3bf715ff62))
    - (steam-client, lang-gen): initial support for protobuf headers, + ([`26decea`](https://github.comgit//saskenuba/SteamHelper-rs/commit/26decea9f60eba1bf4baf512fdf6b9b5f1e8af7b))
    - refactor, fix (steam-language-gen): Uncommented import, reordered packages ([`621c7ad`](https://github.comgit//saskenuba/SteamHelper-rs/commit/621c7ad371e056bdf62c368ae9377252de59b91f))
    - Merge branch 'master' of github.com:saskenuba/SteamHelper-rs ([`a7f33dc`](https://github.comgit//saskenuba/SteamHelper-rs/commit/a7f33dc69ffac55b175d7e071f755a4b917cb9d0))
    - update (steam-lang-gen): Added reference to steam-protobuf package ([`85f2fc8`](https://github.comgit//saskenuba/SteamHelper-rs/commit/85f2fc8741fc1b33d8522adb195a54e5af836b5f))
    - bump (steam-lang-gen): bumped enum dispatch to avoid error ([`8095664`](https://github.comgit//saskenuba/SteamHelper-rs/commit/8095664a56df33723b18612a39e28af149c19874))
    - submodules ([`690b0d1`](https://github.comgit//saskenuba/SteamHelper-rs/commit/690b0d1df9400aa7e23cd613046c6f88f93cb7a9))
    - enums now implement copy ([`d1665f6`](https://github.comgit//saskenuba/SteamHelper-rs/commit/d1665f62bc81693f5055cd323ca8f8790ca93c63))
    - fixup! fix(steam-web-api): added version for local deps ([`72effa1`](https://github.comgit//saskenuba/SteamHelper-rs/commit/72effa1e4d9d32f70250dda3f8b6941c99ddea07))
    - added feature gate for generator deps ([`e3b425d`](https://github.comgit//saskenuba/SteamHelper-rs/commit/e3b425dafa7bf75ab287c23b365bf7a151eb2361))
    - minor fix(steam-language-gen): refactored enums, removed deprecated ([`8fce1c7`](https://github.comgit//saskenuba/SteamHelper-rs/commit/8fce1c7b32661ce4806fab97c836eb6fd7a3a84a))
    - add licenses to crates, ([`7e07992`](https://github.comgit//saskenuba/SteamHelper-rs/commit/7e079927b99f2078f455fa1d85be28465846e9b7))
    - (steam-language-gen) bumped some old enums with serde_repr ([`34db07d`](https://github.comgit//saskenuba/SteamHelper-rs/commit/34db07d2ec084750c5f42a2c8990353ed597c3fa))
    - big refactor and important bug fix. check log for details + ([`9fc8a4e`](https://github.comgit//saskenuba/SteamHelper-rs/commit/9fc8a4e2686ffc6d5cff86822f07a73d2c8f12fa))
    - parser now emit messages with custom attr macro containing emsg, + ([`101b688`](https://github.comgit//saskenuba/SteamHelper-rs/commit/101b688cb8b9a0eb5105ccfc9d465b8c3951a9eb))
    - new basic traits for messages and enums - Clone, PartialEq, Eq ([`d7f5ca8`](https://github.comgit//saskenuba/SteamHelper-rs/commit/d7f5ca8ea12cf0a91619ad48e48ebb114b808270))
    - huge refactor, check log ([`5369d20`](https://github.comgit//saskenuba/SteamHelper-rs/commit/5369d20d9f28cde94b96976dab5e2909f30ddb3f))
    - updated submodules ([`c710c11`](https://github.comgit//saskenuba/SteamHelper-rs/commit/c710c11b80a13bdd2038f481b40a959c9b07d159))
    - minor cleanup on steam-language-gen ([`ba00a2a`](https://github.comgit//saskenuba/SteamHelper-rs/commit/ba00a2af34fd7e2587f4e6bcb1c1d68828b0f5eb))
    - refactored parser and generator for enums/msgs, almost done ([`805f661`](https://github.comgit//saskenuba/SteamHelper-rs/commit/805f661066544c2b70032f2de4062c4a005be6d8))
    - Added Enum parser; ([`566f92d`](https://github.comgit//saskenuba/SteamHelper-rs/commit/566f92d22d71db3d159afb37d0179ba531bd97d5))
    - Parser correctly consumes const, correctly parse three tokens, + ([`bba7bb7`](https://github.comgit//saskenuba/SteamHelper-rs/commit/bba7bb754d6ab3bd4c618fcdbb6d815ff2fdd3fa))
    - more groundwork on .steamd parser, new inflector dep, check log ([`955d388`](https://github.comgit//saskenuba/SteamHelper-rs/commit/955d388d03f5dca2780240bf3db2bf19ca4c8c19))
    - Merge branch 'master' of github.com:saskenuba/SteamHelper-rs ([`95563c1`](https://github.comgit//saskenuba/SteamHelper-rs/commit/95563c1a343a1ca5acae1ee599fa5aad7920d8f2))
    - added SteamKit as submodule, added steammd parser sketch, +log ([`6742ecc`](https://github.comgit//saskenuba/SteamHelper-rs/commit/6742ecc1ac52cfd52f24e06e611ee66b3dca32d5))
</details>

