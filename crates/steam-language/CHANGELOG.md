# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v1.1.0 (2026-10-17)

### New Features

 - <csr-id-2fb7f09e37fe9f49615d641e6b58d0a5d6528d10/> sync EMsg with SteamKit
   EMsg had 351 of SteamKit's message ids, missing ClientHello and the
   ServiceMethod* ids among others. It now has 1908, from SteamKit's
   emsg.steamd at commit eb9a18e (2025-04-21).
   
   - existing variants keep their values; none changed upstream
   - where names share a value, one is kept: a message over its range
     marker (Heartbeat over BaseGM), otherwise the first name
   - 10 ids SteamKit no longer lists are kept (e.g. ClientCancelPurchase,
     ClientGetLobbyList, ClientUDSInviteToGame)
   
   EMsg is non_exhaustive, so adding variants is not breaking.
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
 - 6 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Sync EMsg with SteamKit ([`2fb7f09`](https://github.com/saskenuba/SteamHelper-rs/commit/2fb7f09e37fe9f49615d641e6b58d0a5d6528d10))
    - Parse message headers without panicking ([`3caf1f1`](https://github.com/saskenuba/SteamHelper-rs/commit/3caf1f1803df29d5df382cb77570afca0f2a2037))
</details>

## v1.0.0 (2026-10-11)

### New Features

 - <csr-id-b293671e69a3bb751ab8d9d3f75fc1d474f2861d/> add steam-language with enums, emsg and messages features
   Moves the generated Steam language out of steam-language-gen into one
   crate, split by features so each client only builds what it uses:
   
   - no features: enums (EResult, EAccountType, EUniverse, ...), with only
   serde, bitflags and num-derive as dependencies
   - emsg: EMsg message ids and their packet helpers
   - messages: headers, Msg* structs and their traits, with steam-protobuf
   and bincode. Unstable: not covered by semver.
   
   steam-language-gen keeps the generator and re-exports steam-language
   under the previous paths, so existing code keeps compiling. num-derive
   is now 0.4, which removes 130 non-local impl warnings.

### New Features (BREAKING)

 - <csr-id-cd7631af6c19be5790615e75a5953192d201549f/> regenerate with lite runtime, one feature per proto
   Updates the protobuf generator and regenerates every Steam protobuf:
   
   - protobuf, protobuf-codegen 3.7.2 and protoc-bin-vendored 3.3
   (protoc 36.2); drops the unused protoc-rust and walkdir
   - SteamDatabase/Protobufs submodule moved to the latest upstream, which
   adds 21 protos and drops clientserver_ufs, deviceauth_steamclient and
   star_steamclient
   - each proto is a module behind a cargo feature of the same name, which
   enables the protos it imports; `full` enables all of them. regen
   writes both mod.rs and the features block in Cargo.toml, and clears
   stale output first
   - generated with lite_runtime, without descriptors or reflection. A full
   build goes from ~52s to ~21s, and a crate using only the auth protos
   builds steam-protobuf in ~1s
   - generated code refers to ::protobuf instead of relying on the
   crate-root glob from inside_protobuf; the runtime is re-exported as
   steam_protobuf::protobuf
   - edition 2021, thiserror 2, and a .gitattributes marking the generated
   protos as linguist-generated
   
   steam-mobile, steam-language (messages) and steam-client now name the
   features they use.
 - <csr-id-2b6eda158cbe3d68e5c0abc5d5ad61fc21765e56/> mark enums, EMsg and EMsgError as non_exhaustive
   Steam adds values to these enums over time. Marking them non_exhaustive
   lets new values ship in minor releases of steam-language instead of a
   new major version every time.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release steam-protobuf v0.3.0, steam-language v1.0.0, steamid-parser v1.0.0, steam-mobile v0.6.0 ([`55df24d`](https://github.com/saskenuba/SteamHelper-rs/commit/55df24dbca5be6f3b26cb2979ca22f28614956c3))
    - Regenerate with lite runtime, one feature per proto ([`cd7631a`](https://github.com/saskenuba/SteamHelper-rs/commit/cd7631af6c19be5790615e75a5953192d201549f))
    - Mark enums, EMsg and EMsgError as non_exhaustive ([`2b6eda1`](https://github.com/saskenuba/SteamHelper-rs/commit/2b6eda158cbe3d68e5c0abc5d5ad61fc21765e56))
    - Add steam-language with enums, emsg and messages features ([`b293671`](https://github.com/saskenuba/SteamHelper-rs/commit/b293671e69a3bb751ab8d9d3f75fc1d474f2861d))
</details>

