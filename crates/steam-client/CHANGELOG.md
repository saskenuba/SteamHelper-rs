# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.2.0 (2026-10-21)

### Documentation

 - <csr-id-40dcb71581b1b3181cafd67898205cb0ecc76bc8/> trade_notifications points to steam-mobile's login_with_client
   For the web session that steam-trading fetches the offers with, and to
   steam-mobile's web_session_bot example, which sets it up.

### New Features

 - <csr-id-40bd72cee586bd2b910272e3d26faf0c43adbf09/> AccessToken::issued_at
   When Steam issued a token (its iat claim), if it says. It is Steam's
   clock, as expires_at is: the time between the two is how long the
   token lasts, whatever the local clock says. steam-mobile renews web
   sessions by it.
   
   A token whose expiry is out of range is ClientError::InvalidToken
   instead of a panic.
 - <csr-id-aa8f6f5582c0fc3a6d9468f7472dbdb698770a27/> job messages
   Requests that aren't service methods but are answered by one message
   carrying their job id, such as ClientRequestFreeLicense, answered by
   ClientRequestFreeLicenseResponse.
   
   - job::JobMessage names one by a marker type, as ServiceMethod does:
     the crate's own in job::messages, sent with
     call_job_message(request), any other with
     call_job::<Message>(request), on SteamClient and SteamHandle. At
     most 30 s; on a handle, concurrently with the other calls
   - an answer whose result isn't OK is ClientError::JobMessage, with the
     request and the result, such as RateLimitExceeded for a free license
   - service methods and job messages are one kind of exchange in the
     sans-IO core: job ids, matching and timeouts are shared
   - what Steam pushes because of a request, such as the ClientLicenseList
     after a free license, stays an event, not part of the answer
   - reading the answers allocates nothing (tests/zero_alloc.rs)
   - example free_license: ClientRequestFreeLicense for the app ids given,
     or with BY_HAND=1 through a marker type declared in the example

### Bug Fixes

 - <csr-id-fb417377ac79a43d39ba0978981e31a5ee38d200/> an unreadable GC message counts as dropped
   A message from a game coordinator whose envelope can't be read can't be
   routed: it was logged and lost. Since it may have been any channel's,
   every channel now counts it as dropped, so its reader sees
   GcEvent::Lagged.

### New Features (BREAKING)

 - <csr-id-7cc393a45dabfa223531dc1a3b809f1f89ac8085/> CS2 inventory only while it is current
   Cs2::inventory() gives the account's inventory only while it is the
   game coordinator's current one, and None otherwise: before the game
   coordinator sent it, and from Cs2Event::InventoryUnavailable (the
   session ended, messages were dropped or couldn't be read, the cache was
   unsubscribed or is another version) until Cs2Event::InventoryLoaded.
   It first reads what came since, as events, so the answer is as of the
   latest message.
   
   - what changed while the inventory was unavailable comes as item events
     just before InventoryLoaded
   - Cs2 asks the game coordinator to send the cache again
     (CacheSubscriptionRefresh, as the game does) when its copy may be old
   - InventoryLoaded and InventoryUnavailable are never dropped, like the
     session events
   - example cs2_profile prints the new events
   
   BREAKING: Cs2::inventory takes &mut self and returns
   Result<Option<&Inventory>, ClientError> (Closed once the client
   stopped, GcRefused if the game coordinator refused the session); new
   Cs2Event::InventoryUnavailable.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release.
 - 3 days passed between releases.
 - 5 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Log in through a Steam client (feature cm) ([`40dcb71`](https://github.com/saskenuba/SteamHelper-rs/commit/40dcb71581b1b3181cafd67898205cb0ecc76bc8))
    - CS2 inventory only while it is current ([`7cc393a`](https://github.com/saskenuba/SteamHelper-rs/commit/7cc393a45dabfa223531dc1a3b809f1f89ac8085))
    - An unreadable GC message counts as dropped ([`fb41737`](https://github.com/saskenuba/SteamHelper-rs/commit/fb417377ac79a43d39ba0978981e31a5ee38d200))
    - AccessToken::issued_at ([`40bd72c`](https://github.com/saskenuba/SteamHelper-rs/commit/40bd72cee586bd2b910272e3d26faf0c43adbf09))
    - Job messages ([`aa8f6f5`](https://github.com/saskenuba/SteamHelper-rs/commit/aa8f6f5582c0fc3a6d9468f7472dbdb698770a27))
</details>

## v0.1.0 (2026-10-18)

### Chore

 - <csr-id-11d213edc03f1289fa139d0ebb8b60b4fcde63b8/> publish
   steam-client-lib and steam-cryptography are free on crates.io. Both get
   a license, repository and description, steam-client-lib's path
   dependencies a version, and env_logger a version instead of *, which
   crates.io rejects.
 - <csr-id-8cba64f4e70cd363e2fb272f9fe16df5bb95c147/> remove unused dependencies
 - <csr-id-5b9d6ada9494d5e7cb76ccf5a2a0c883d80dfa6f/> set not to publish wip crate

### Documentation

 - <csr-id-e8ab0f5e9565a20b9928bd3f88efcefe8f0a0671/> game coordinator examples
   - gc::cs2's page: what a session gives and needs (CS2's free license,
     one request at a time), then the whole flow over a spawned handle:
     connect, inspect a link, a friend's profile, a match from its share
     code, the inventory, the events. What isn't covered: other accounts'
     inventories, items inside storage units, changing the account.
   - GcChannel, for any game: play it, open the channel, greet the
     coordinator on a timer until it welcomes the account and again after
     SessionLost, read its messages. Written with CS2's protos and
     compiled with the cs2 feature only.
   - InspectLink (offline: inventory, market and embedded links),
     Inventory, PlayerProfile, MatchList, MatchInfo, Cs2::next_event.
 - <csr-id-2ffe74625d5a691b7eaff9abe658c06a7eb2f8a6/> examples
   Anonymous, run live: events (what Steam sends), service_call (a call
   before logon and a method the crate doesn't wrap, through a marker
   type), reconnect (a bot's lifecycle and Ctrl-C), task_free (the
   next_event loop without a task) and many_accounts (memory per client).
   
   Account, not run live yet: login (first logon with Steam Guard, saves
   the refresh token), chat_bot (an echo bot), trade_notifications (the
   push that tells when trade offers change).
   
   examples/README.md lists them with their variables and files.
 - <csr-id-fb87360214c2f6d1319f467b82b27706ae157111/> added CHANGELOG.md, modified manifest versions
   We are now using cargo-smart-release to organize releases among with
   conventional commits;

### New Features

 - <csr-id-db764dba59b54e85d7b54294aea33f855c00e062/> CS2 profiles, matches, inventory and events
   Cs2 now does more than inspect links, each request paired with its
   answer, one at a time with the same 1.1 s cooldown and 10 s timeout:
   - request_matchmaking_stats (9109 → 9110), kept as matchmaking_stats;
     player_profile (9127 → 9128, paired by the first profile's account,
     as node-globaloffensive): PlayerProfile with ranks, level, XP,
     commendations, penalty, medals.
   - match_info from a ShareCode, recent_matches, live_match, live_matches
     (→ 9139 MatchList, paired by msgrequestid as ValvePython's csgo):
     MatchInfo with map, score, duration, players, demo URL, share code.
   - ShareCode reads and writes CSGO-xxxxx-… codes and download links (as
     globaloffensive-sharecode and akiver's csgo-sharecode).
   - inventory(): the account's items from the welcome's caches, kept up
     to date by the shared-object messages (create, update, destroy,
     update multiple, cache subscribed); InventoryItem reads the paint
     kit, pattern, float, StatTrak, name, stickers, storage unit and
     position from the attributes, as node-globaloffensive does.
   - next_event / try_next_event: what no request waits for, as Cs2Event
     (session lost, welcomed again, stats, inventory loaded, item
     acquired / changed / removed, other messages, lagged), queued up to
     EVENT_CAPACITY; next_event greets again while the session is lost.
     Answers that come late are events now, no longer dropped.
   - ClientError gains NotIndividual.
   
   Example cs2_profile (compiled only, not run live yet).
 - <csr-id-9308a6437e02036f18de7aa62a4905e9965b2c40/> CS2 inspect links and item details
   Behind a `cs2` feature (it enables steam-protobuf's `csgo`), gc::cs2:
   - InspectLink: inventory (S…A…D…) and market (M…A…D…) links, full or
     bare, with %20 or a space; from_template fills Steam's
     %owner_steamid% and %assetid%. Links that carry the item itself (hex,
     masked with a key byte, checksum verified, as node-globaloffensive
     reads them) decode offline.
   - ItemDetails over CEconItemPreviewDataBlock: pattern (paintseed),
     float (paintwear's bits), paint index, StatTrak, name tag, stickers,
     keychains; the proto stays reachable.
   - Cs2::connect plays CS2 and says hello (as node-globaloffensive: its
     version, 0.5 s then backing off to 60 s) until the GC welcomes the
     account. Cs2::inspect asks for one item at a time, 1.1 s apart (as
     csfloat), matches the answer by item id, gives up after 10 s, and
     greets again after the session was lost.
   - ClientError gains GcTimeout, GcRefused, InspectLink and ItemDetails.
   
   Example cs2_inspect (compiled only, not run live yet).
 - <csr-id-8f56352c0fce0747467fad9b6a8936574121af2d/> game coordinator transport
   A game's coordinator (GC) talks to an account that plays the game,
   through the CM connection: messages go as ClientToGC (CMsgGCClient,
   CM header routed to the app) and come back as ClientFromGC, as
   SteamKit's SteamGameCoordinator and node-steam-user's sendToGC do.
   Nothing here is specific to a game.
   - set_games_played (ClientGamesPlayedWithDataBlob, as node-steam-user),
     on SteamHandle<User> and SteamClient<LoggedOn<User>>: kept and sent
     again after each relog, as the persona state.
   - SteamHandle<User>::game_coordinator(app_id) -> GcChannel: the task
     routes that app's messages to the channel instead of Events. It
     never waits for the reader (drops are counted, GcEvent::Lagged), and
     tells each channel when the session is lost (GcEvent::SessionLost),
     so the game greets its GC again. A channel doesn't keep the client
     running; a second one for the same app replaces the first.
   - Task-free: send_to_gc, and RawMessage::gc_message to read them.
   - GcMessage: the type without the protobuf flag, and the body, a slice
     of the read buffer: reading a GC message allocates nothing up to the
     routing (zero_alloc case).
   
   tokio's minimum is now 1.22, for mpsc::WeakUnboundedSender.

### Other

 - <csr-id-c91f45591d29ee93bbb41fc29b04793e5c321c88/> initial work on main loop of steam client
 - <csr-id-0d7c7ef51a52b37bb6b6530b3e95b922dd303e7b/> steam-api is now steam-client

### Refactor

 - <csr-id-a398f7c167cfae9085ffd3f2ccaeac2b4273c6eb/> content_manager is cm_list
   CM is Connection Manager, and the module only fetches the CM list.
 - <csr-id-26c500868085f81f35f3cedb04c7533a41622a65/> moved modules around to make it more...
   readable
   
   * Replaced web-api module with steam-web-api crate, along with static api client
   * Added tokio-compat2 crate to make tokio 1.0 compatible
   * Replaced simple reader with tokio FramedRead for increased efficiency
   * Renamed Msg<T> to ClientMessage<T>
   * Added ClientMessage support for Protobuf header and body;

### New Features (BREAKING)

 - <csr-id-258e6c3fe4ff60ba3ac041bf9af9a942b7ddc432/> account logon, sans-IO core and spawned client
   Account logon over the CM connection, as SteamKit does it: a token from
   IAuthenticationService (non-authed service calls, platform SteamClient),
   then ClientLogon with the refresh token, never the password.
   - AuthSession for Steam Guard (device code from a shared secret, device
     or email code), login, login_with_refresh_token, renew_tokens and
     web_access_token on LoggedOn<User>.
   - Account logons ask for the new chat (chat_mode 2); set_persona_state.
   
   The protocol is now a state machine with no IO (protocol/): framing,
   the channel encryption handshake, encryption, Multi unpacking, job
   matching and heartbeat deadlines, with time passed in. Connection is
   the tokio driver: one owner, no split socket, no lock, no heartbeat
   task; calls are cancel-safe and frames are limited to 16 MiB.
 - <csr-id-28d13d7aeb44d192eb68990d925ec1928e3e1e99/> anonymous logon, zero-alloc deserialization
   steam-client didn't compile and stopped after half of the encryption
   handshake. It now logs on anonymously to a CM over TCP, verified live.
   
   - SteamClient::connect() (first servers of GetCMList) or connect_to()
     encrypts the channel and sends ClientHello: SteamClient<Connected>.
     login_anonymous() moves to SteamClient<LoggedOn>, with steam_id,
     session_id, cell_id, heartbeat_interval and public_ip; log_off()
     sends ClientLogOff and waits for ClientLoggedOff or the socket to close
   - the LoggedOn state owns the heartbeat task; every message sent after
     logon carries the SteamID and session id
   - FrameCodec frames the handshake, then EncryptedCodec takes over the
     same buffer (Framed::map_codec), encrypting with the HMAC IV
   - unknown EMsgs and messages that don't parse are logged and skipped;
     Multi messages (gzip, raw or nested) are unpacked in order
   - deserialization doesn't allocate: frames are decrypted in place, and
     packets, bodies and Multi contents are Bytes slices of the frame. Gzip
     is inflated with miniz_oxide into one buffer of the declared size,
     checked against the most deflate can expand. tests/zero_alloc.rs
     asserts it with a counting allocator. Protobuf bodies still allocate
   - example anonymous_logon, and a captured ClientLogOnResponse fixture
     (public IP replaced with a documentation address)
   - removed the websockets connection code, which didn't compile
   - ClientError replaces ConnectionError; tokio-util 0.7
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

### Refactor (BREAKING)

 - <csr-id-4051644572ae76371ae7f3a44ff271d90577743c/> the package is steam-client-lib
   steam-client on crates.io is someone else's 0.0.0 placeholder. The
   library keeps its name, so code still uses steam_client:: and
   RUST_LOG=steam_client; only cargo's -p changes.
 - <csr-id-c0354083bed7367bc714b54515b43d9f5e194d64/> the crate is steam-cryptography
   steam-crypto on crates.io is an unrelated 2015 crate, so this one can't
   be published under that name. The directory, package and library are
   steam-cryptography (steam_cryptography::).
 - <csr-id-4fe4a380af3617fbcfdc0205dc15eab16c9c3b03/> INSPECT_COOLDOWN and INSPECT_TIMEOUT are REQUEST_*
   Cs2's cooldown and timeout apply to every request kind (profiles,
   matches, stats), not inspect links only: REQUEST_COOLDOWN and
   REQUEST_TIMEOUT. Nothing is published, so no deprecated aliases.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 26 commits contributed to the release over the course of 2102 calendar days.
 - 19 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#12](https://github.com/saskenuba/SteamHelper-rs/issues/12)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#12](https://github.com/saskenuba/SteamHelper-rs/issues/12)**
    - Fix cargo doc ([`208a6ad`](https://github.com/saskenuba/SteamHelper-rs/commit/208a6ad5e2484f9cea101110dc8e29d220475821))
 * **Uncategorized**
    - Publish ([`11d213e`](https://github.com/saskenuba/SteamHelper-rs/commit/11d213edc03f1289fa139d0ebb8b60b4fcde63b8))
    - Content_manager is cm_list ([`a398f7c`](https://github.com/saskenuba/SteamHelper-rs/commit/a398f7c167cfae9085ffd3f2ccaeac2b4273c6eb))
    - The package is steam-client-lib ([`4051644`](https://github.com/saskenuba/SteamHelper-rs/commit/4051644572ae76371ae7f3a44ff271d90577743c))
    - The crate is steam-cryptography ([`c035408`](https://github.com/saskenuba/SteamHelper-rs/commit/c0354083bed7367bc714b54515b43d9f5e194d64))
    - Game coordinator examples ([`e8ab0f5`](https://github.com/saskenuba/SteamHelper-rs/commit/e8ab0f5e9565a20b9928bd3f88efcefe8f0a0671))
    - INSPECT_COOLDOWN and INSPECT_TIMEOUT are REQUEST_* ([`4fe4a38`](https://github.com/saskenuba/SteamHelper-rs/commit/4fe4a380af3617fbcfdc0205dc15eab16c9c3b03))
    - CS2 profiles, matches, inventory and events ([`db764db`](https://github.com/saskenuba/SteamHelper-rs/commit/db764dba59b54e85d7b54294aea33f855c00e062))
    - CS2 inspect links and item details ([`9308a64`](https://github.com/saskenuba/SteamHelper-rs/commit/9308a6437e02036f18de7aa62a4905e9965b2c40))
    - Game coordinator transport ([`8f56352`](https://github.com/saskenuba/SteamHelper-rs/commit/8f56352c0fce0747467fad9b6a8936574121af2d))
    - Examples ([`2ffe746`](https://github.com/saskenuba/SteamHelper-rs/commit/2ffe74625d5a691b7eaff9abe658c06a7eb2f8a6))
    - Account logon, sans-IO core and spawned client ([`258e6c3`](https://github.com/saskenuba/SteamHelper-rs/commit/258e6c3fe4ff60ba3ac041bf9af9a942b7ddc432))
    - Remove unused dependencies ([`8cba64f`](https://github.com/saskenuba/SteamHelper-rs/commit/8cba64f4e70cd363e2fb272f9fe16df5bb95c147))
    - Anonymous logon, zero-alloc deserialization ([`28d13d7`](https://github.com/saskenuba/SteamHelper-rs/commit/28d13d7aeb44d192eb68990d925ec1928e3e1e99))
    - Regenerate with lite runtime, one feature per proto ([`cd7631a`](https://github.com/saskenuba/SteamHelper-rs/commit/cd7631af6c19be5790615e75a5953192d201549f))
    - Set not to publish wip crate ([`5b9d6ad`](https://github.com/saskenuba/SteamHelper-rs/commit/5b9d6ada9494d5e7cb76ccf5a2a0c883d80dfa6f))
    - Added CHANGELOG.md, modified manifest versions ([`fb87360`](https://github.com/saskenuba/SteamHelper-rs/commit/fb87360214c2f6d1319f467b82b27706ae157111))
    - (trading, client): fixed tappet rename ([`ce6172a`](https://github.com/saskenuba/SteamHelper-rs/commit/ce6172aa2c57d917d5345240a51c4e51a4b34fff))
    - Initial work on main loop of steam client ([`c91f455`](https://github.com/saskenuba/SteamHelper-rs/commit/c91f45591d29ee93bbb41fc29b04793e5c321c88))
    - Moved modules around to make it more... ([`26c5008`](https://github.com/saskenuba/SteamHelper-rs/commit/26c500868085f81f35f3cedb04c7533a41622a65))
    - (steam-client, lang-gen): initial support for protobuf headers, + ([`26decea`](https://github.com/saskenuba/SteamHelper-rs/commit/26decea9f60eba1bf4baf512fdf6b9b5f1e8af7b))
    - Bump, fix (steam-client, steam-protobuf): multiple dep bump and fix ([`d11a7bb`](https://github.com/saskenuba/SteamHelper-rs/commit/d11a7bb751b2314266ebeb3d133381711b446cc2))
    - Merge #9 from Nexure, minor fixes ([`c3c80e4`](https://github.com/saskenuba/SteamHelper-rs/commit/c3c80e4548aee3571130a34f58123155719414fe))
    - Bump (steam-lang-gen): bumped enum dispatch to avoid error ([`497dccd`](https://github.com/saskenuba/SteamHelper-rs/commit/497dccd6588fece7e9fd83837849526c0d4946ab))
    - Fixes (steam-client): pinned rust-protobuf dep to 2.2.0 ([`3fc422a`](https://github.com/saskenuba/SteamHelper-rs/commit/3fc422a52b14eea0ed9f0c2886d27df534175723))
    - Steam-api is now steam-client ([`0d7c7ef`](https://github.com/saskenuba/SteamHelper-rs/commit/0d7c7ef51a52b37bb6b6530b3e95b922dd303e7b))
</details>

