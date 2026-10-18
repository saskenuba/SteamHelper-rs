# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.1 (2026-10-18)

### Chore

 - <csr-id-8cba64f4e70cd363e2fb272f9fe16df5bb95c147/> remove unused dependencies

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 1 commit contributed to the release over the course of 1 calendar day.
 - 1 day passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Remove unused dependencies ([`8cba64f`](https://github.com/saskenuba/SteamHelper-rs/commit/8cba64f4e70cd363e2fb272f9fe16df5bb95c147))
</details>

## 0.2.0 (2026-10-17)

### New Features

 - <csr-id-3caf1f1803df29d5df382cb77570afca0f2a2037/> parse message headers without panicking
   Protobuf headers were split at size_of::<CMsgProtoBufHeader>() (480
   bytes) instead of reading their u32 length prefix, and EMsg parsing
   didn't mask the protobuf flag, so every protobuf packet failed. Short
   input panicked everywhere.
   
   - MessageHeaderWrapper::parse(packet) returns the EMsg, the header and
   the body; write(emsg, out) sets the protobuf flag and length prefix
   - MessageHeaderExt::parse and MessageBodyExt::parse return the parsed
   value and the rest, or a MessageError. The derives read through
   bincode::deserialize_from, so they consume exactly what bincode reads
   - removed DeserializableBytes, split_from_bytes and the wrapper's
   SerializableBytes impl
   - ExtendedMessageHeader header_size defaults to 36, as in SteamKit
   - emsg: EMsg::split_raw; from_raw_message masks the protobuf flag and
   errors on short input; EMsgError implements Display and Error
   - tests with captured packets, round trips and short input
   
   The `messages` feature is unstable, so its API changes here are not
   semver breaking for steam-language. steam-language-gen-derive and
   steam-language-gen change incompatibly and need a minor (0.x) bump.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 1614 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release steam-language-gen-derive v0.2.0, steam-language v1.1.0, steam-language-gen v0.2.0 ([`df6a226`](https://github.com/saskenuba/SteamHelper-rs/commit/df6a2269279754ba717febabe8a6e3d625e9e3eb))
    - Parse message headers without panicking ([`3caf1f1`](https://github.com/saskenuba/SteamHelper-rs/commit/3caf1f1803df29d5df382cb77570afca0f2a2037))
</details>

## 0.1.2 (2022-05-17)

<csr-id-843013c43386a837de6b816f65ab2e520677bab4/>
<csr-id-7e079927b99f2078f455fa1d85be28465846e9b7/>

### Documentation

 - <csr-id-fb87360214c2f6d1319f467b82b27706ae157111/> added CHANGELOG.md, modified manifest versions
   We are now using cargo-smart-release to organize releases among with
   conventional commits;

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 10 commits contributed to the release over the course of 898 calendar days.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release steam-language-gen-derive v0.1.2, steam-protobuf v0.1.2, steam-language-gen v0.1.2, steam-totp v0.2.2, steamid-parser v0.2.1, steam-mobile v0.3.0 ([`cf773b0`](https://github.com/saskenuba/SteamHelper-rs/commit/cf773b07e0ae68376bf960d12f94ecb96afa9211))
    - Added CHANGELOG.md, modified manifest versions ([`fb87360`](https://github.com/saskenuba/SteamHelper-rs/commit/fb87360214c2f6d1319f467b82b27706ae157111))
    - (steam-lang-derive): bump to 0.1.1 ([`05152ce`](https://github.com/saskenuba/SteamHelper-rs/commit/05152ce6395cf5d99647a62b3becdac60467e89a))
    - Renamed traits, impl serializablebytes for proto.. ([`843013c`](https://github.com/saskenuba/SteamHelper-rs/commit/843013c43386a837de6b816f65ab2e520677bab4))
    - Fix (steam-lang-gen): message structs are now packed, and copy types ([`10aa5f8`](https://github.com/saskenuba/SteamHelper-rs/commit/10aa5f8cffe944cb9ec82ea392db0e3bf715ff62))
    - Add licenses to crates, ([`7e07992`](https://github.com/saskenuba/SteamHelper-rs/commit/7e079927b99f2078f455fa1d85be28465846e9b7))
    - Big refactor and important bug fix. check log for details + ([`9fc8a4e`](https://github.com/saskenuba/SteamHelper-rs/commit/9fc8a4e2686ffc6d5cff86822f07a73d2c8f12fa))
    - Parser now emit messages with custom attr macro containing emsg, + ([`101b688`](https://github.com/saskenuba/SteamHelper-rs/commit/101b688cb8b9a0eb5105ccfc9d465b8c3951a9eb))
    - Huge refactor, check log ([`5369d20`](https://github.com/saskenuba/SteamHelper-rs/commit/5369d20d9f28cde94b96976dab5e2909f30ddb3f))
    - Added custom derive for steam messages (to de/serialize) w/ bincode ([`2d19e42`](https://github.com/saskenuba/SteamHelper-rs/commit/2d19e42ca70276199019292cf040d36ee620ff56))
</details>

