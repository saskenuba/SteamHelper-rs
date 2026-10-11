# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 1.0.0 (2026-10-11)

### New Features

 - <csr-id-ef0f99b8c0eddb28d5dd03e3102acaf5e9106fc1/> deserialize SteamID from strings in human readable formats
   The Steam Web API returns steamids as strings, which failed to
   deserialize. Human readable formats such as JSON now accept a number, a
   string with any Steam64 number, or a string in any format accepted by
   str::parse. Non human readable formats such as bincode still read a
   plain u64, and serialization is unchanged.
 - <csr-id-d2eb0e84cca7bcfc7121db577b07f0d8257acdc6/> implement Display, From<u64> and From<SteamID> for u64
   Display writes the Steam64 number, and the conversions are the same as
   from_steam64 and to_steam64, so all of them agree with how SteamID is
   serialized.
 - <csr-id-f084cbbf7792e43fe3daafc19e85fdc9b141ae8b/> add to_steam3_string
   Renders [T:U:W] the way Steam does: c and L for clan chats and lobbies,
   i for account types without a letter, and the instance as [T:U:W:I] for
   anonymous game servers, multiseat accounts and non-desktop individual
   accounts.
   
   It is a new name rather than a change to to_steam3, which keeps
   returning the account id as a number.
 - <csr-id-43b898579405068b5462ea810562380e66502ed0/> add from_account_id for public individual accounts
   A shortcut for from_steam3(account_id, None, None), which is how every
   caller in the workspace builds a SteamID from an account id.
 - <csr-id-38a84c5cb8dd73d5b63a41c15e86f4c0948089b1/> return None from to_steam2 for non-individual accounts
   Steam2 has no account type or instance, so rendering a clan or game
   server as STEAM_X:Y:Z produced an id that parses back as a different,
   individual account. to_steam2 now returns Option<String> and is None for
   any account type other than Individual.
   
   to_steam2 has not been released yet, so this changes no published API.
 - <csr-id-56a108d54c3428739ae7bf8e97b5fac751520a6f/> add account_id, instance, account_type and universe getters
   account_id returns the same value as to_steam3, as a u32. account_type
   and universe return None when the bits hold a value with no known enum
   variant, since from_steam64 accepts any u64.
 - <csr-id-67b65b57c1f783b8577347db65fabf670d2af1de/> add FromStr with ParseSteamIDError, add find, deprecate parse
   SteamID now implements FromStr for strings that are exactly one
   Steam64, Steam3 or Steam2 id, returning a ParseSteamIDError (built with
   thiserror) that says whether the format is unknown, the Steam3 account
   type is unsupported or the account is out of range.
 - <csr-id-3d47c96166507809386d79fc00e76f163dd5298a/> parse and format Steam2 ids
   parse now reads STEAM_X:Y:Z as a public individual account, with both
   STEAM_0 and STEAM_1 meaning the public universe. It is tried after
   Steam3 and Steam64, so inputs that already parsed are unaffected, and
   an account number that doesn't fit in 31 bits is skipped.
   
   Adds to_steam2, which renders the public universe as STEAM_0.

### Other

 - <csr-id-75ec4dcf47f0d9bbbc07868692183bd62bd55377/> steamid-parser 1.0

### Refactor

 - <csr-id-7a3a8692fe280245bf59f6ac110e330c5d39c18a/> depend on steam-language enums instead of steam-language-gen
   EUniverse and EAccountType are the same types steam-language-gen
   re-exports, so callers are unaffected. With no steam-language features
   enabled, steamid-parser's dependency tree goes from ~99 crates to 16 and
   no longer builds steam-protobuf.
 - <csr-id-ed733d62b45ba8b7fa74906a082514aec9c21eab/> store SteamID as u64 and drop bitvec, num, lazy_static and regex
   SteamID is now a single u64 read with shifts and masks instead of five
   heap allocated BitVecs, and parsing uses a small scanner instead of
   regexes. The struct goes from 104 to 8 bytes, steam64 roundtrips from
   ~175ns to ~4ns and parse from ~330ns to ~27ns.
   
   Behavior is unchanged, including to_steam64 clearing the highest bit.
   The only difference is that non-ascii digits are no longer matched,
   which previously panicked when converted.

### Test

 - <csr-id-feec3015fd282f2e6b74018ac53a386edd696bf6/> cover current conversion and parsing behavior
   Adds tests for steam64/steam3 roundtrips, every universe and account
   type, parse priority and first-match rules, malformed and invalid
   input, and serde. Inputs that currently panic are pinned with
   should_panic so a future fix shows up as a test change.

### New Features (BREAKING)

 - <csr-id-2b6eda158cbe3d68e5c0abc5d5ad61fc21765e56/> mark enums, EMsg and EMsgError as non_exhaustive
   Steam adds values to these enums over time. Marking them non_exhaustive
   lets new values ship in minor releases of steam-language instead of a
   new major version every time.
 - <csr-id-d79251efe10f2e43ebcf01ddd94697eda01b749a/> derive Copy, Eq, Hash and Ord, take self by value
   SteamID is an 8 byte value, so it is now Copy and can be used as a
   HashMap/HashSet key or sorted. Ordering follows the Steam64 value.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 14 commits contributed to the release.
 - 1608 days passed between releases.
 - 14 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Depend on steam-language enums instead of steam-language-gen ([`7a3a869`](https://github.com/saskenuba/SteamHelper-rs/commit/7a3a8692fe280245bf59f6ac110e330c5d39c18a))
    - Mark enums, EMsg and EMsgError as non_exhaustive ([`2b6eda1`](https://github.com/saskenuba/SteamHelper-rs/commit/2b6eda158cbe3d68e5c0abc5d5ad61fc21765e56))
    - Steamid-parser 1.0 ([`75ec4dc`](https://github.com/saskenuba/SteamHelper-rs/commit/75ec4dcf47f0d9bbbc07868692183bd62bd55377))
    - Deserialize SteamID from strings in human readable formats ([`ef0f99b`](https://github.com/saskenuba/SteamHelper-rs/commit/ef0f99b8c0eddb28d5dd03e3102acaf5e9106fc1))
    - Implement Display, From<u64> and From<SteamID> for u64 ([`d2eb0e8`](https://github.com/saskenuba/SteamHelper-rs/commit/d2eb0e84cca7bcfc7121db577b07f0d8257acdc6))
    - Add to_steam3_string ([`f084cbb`](https://github.com/saskenuba/SteamHelper-rs/commit/f084cbbf7792e43fe3daafc19e85fdc9b141ae8b))
    - Add from_account_id for public individual accounts ([`43b8985`](https://github.com/saskenuba/SteamHelper-rs/commit/43b898579405068b5462ea810562380e66502ed0))
    - Return None from to_steam2 for non-individual accounts ([`38a84c5`](https://github.com/saskenuba/SteamHelper-rs/commit/38a84c5cb8dd73d5b63a41c15e86f4c0948089b1))
    - Add account_id, instance, account_type and universe getters ([`56a108d`](https://github.com/saskenuba/SteamHelper-rs/commit/56a108d54c3428739ae7bf8e97b5fac751520a6f))
    - Add FromStr with ParseSteamIDError, add find, deprecate parse ([`67b65b5`](https://github.com/saskenuba/SteamHelper-rs/commit/67b65b57c1f783b8577347db65fabf670d2af1de))
    - Derive Copy, Eq, Hash and Ord, take self by value ([`d79251e`](https://github.com/saskenuba/SteamHelper-rs/commit/d79251efe10f2e43ebcf01ddd94697eda01b749a))
    - Parse and format Steam2 ids ([`3d47c96`](https://github.com/saskenuba/SteamHelper-rs/commit/3d47c96166507809386d79fc00e76f163dd5298a))
    - Store SteamID as u64 and drop bitvec, num, lazy_static and regex ([`ed733d6`](https://github.com/saskenuba/SteamHelper-rs/commit/ed733d62b45ba8b7fa74906a082514aec9c21eab))
    - Cover current conversion and parsing behavior ([`feec301`](https://github.com/saskenuba/SteamHelper-rs/commit/feec3015fd282f2e6b74018ac53a386edd696bf6))
</details>

## 0.2.1 (2022-05-17)

<csr-id-3fc27a6f233942c2da58525491c62c65a1a99ada/>
<csr-id-14404f4fd83c4c74893e3888693398d98bc3f199/>
<csr-id-fc097e8451163de3cc88dece43438d100ec0ddb3/>

### Documentation

 - <csr-id-fb87360214c2f6d1319f467b82b27706ae157111/> added CHANGELOG.md, modified manifest versions
   We are now using cargo-smart-release to organize releases among with
   conventional commits;

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 11 commits contributed to the release over the course of 761 calendar days.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release steamid-parser v0.2.1, steam-mobile v0.3.0 ([`fd04253`](https://github.com/saskenuba/SteamHelper-rs/commit/fd0425344eb5d24093154320cc0ed81bf82a0b1a))
    - Release steamid-parser v0.2.1, steam-mobile v0.3.0 ([`ea7632d`](https://github.com/saskenuba/SteamHelper-rs/commit/ea7632d2fe5fcd85b48315f246f815afba88e62e))
    - Release steam-language-gen-derive v0.1.2, steam-protobuf v0.1.2, steam-language-gen v0.1.2, steam-totp v0.2.2, steamid-parser v0.2.1, steam-mobile v0.3.0 ([`cf773b0`](https://github.com/saskenuba/SteamHelper-rs/commit/cf773b07e0ae68376bf960d12f94ecb96afa9211))
    - Added CHANGELOG.md, modified manifest versions ([`fb87360`](https://github.com/saskenuba/SteamHelper-rs/commit/fb87360214c2f6d1319f467b82b27706ae157111))
    - Updated README with badges and minor fixes ([`14404f4`](https://github.com/saskenuba/SteamHelper-rs/commit/14404f4fd83c4c74893e3888693398d98bc3f199))
    - (steamid): added repo, license and desc, upload to crates.io ([`032c3c9`](https://github.com/saskenuba/SteamHelper-rs/commit/032c3c9b8fc3e64458105f26737a10cf277defc3))
    - SteamID has now serde support by default ([`fc097e8`](https://github.com/saskenuba/SteamHelper-rs/commit/fc097e8451163de3cc88dece43438d100ec0ddb3))
    - Fix and tests(steamid_parser): fix issue with even SteamIDs ([`4672761`](https://github.com/saskenuba/SteamHelper-rs/commit/4672761a7c3a95e9ca6f94940c47ddfec0e3c4cc))
    - Rustfmt reformat crate ([`3fc27a6`](https://github.com/saskenuba/SteamHelper-rs/commit/3fc27a6f233942c2da58525491c62c65a1a99ada))
    - Set pub visibility of steamid parser methods ([`c5ef622`](https://github.com/saskenuba/SteamHelper-rs/commit/c5ef622674362e06b567254d2259283fd5b37425))
    - Moved steamid to its own crate ([`7087a75`](https://github.com/saskenuba/SteamHelper-rs/commit/7087a75490831b8e49a434b6946db4daa630efb9))
</details>

