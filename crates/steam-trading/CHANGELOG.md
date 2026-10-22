# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### New Features (BREAKING)

 - <csr-id-8ad274fbaee8a5df823c5bbdc18580f8835fbc0b/> accept_offer_and_find_items takes the fingerprints, reads no partner inventory
   SteamTradeManager::accept_offer_and_find_items(offer_id, &fingerprints,
   &mut Cs2) (feature cs2): the caller gives the pattern and wear of the
   partner's copies, by their asset id in the offer, as read from the
   partner's inventory before the trade (Inventory::fingerprints). Offers
   and trade history carry none (seen live 2026-10-22), and the call no
   longer reads the partner's inventory.
   
   - snapshot of the game coordinator's item ids, accept, wait for the new
     ones, then places items as find_received_items does
   - Located has three variants instead of five: Exact (same pattern as
     the fingerprint, wear within 1e-6), AnyOfKind (a new item of its
     classid in our web inventory, for an item without a fingerprint:
     every commodity) and NotFound; nothing is placed by elimination or
     guessed among unknown kinds any more
   - web reads: none when every CS2 item received has a fingerprint, else
     one of our own context 16
   - AcceptedOffer::known_ids and TradeError::AcceptedButItemsNotFound
     { known_ids, .. }: the game coordinator's item ids right before the
     accept, to pass to find_received_items to look again
   - a failed read of our web inventory after the accept is now
     AcceptedButItemsNotFound (is_too_many_requests() tells a 429) instead
     of items placed without kinds
   
   Not verified against live Steam.

### New Features

 - <csr-id-21ec6420cd94a1c59c62dd49b3e9a307fe9ed1c1/> where the items of an offer the partner accepted went
   SteamTradeManager::find_received_items(&offer, &known_ids, &fingerprints,
   &mut Cs2) (feature cs2), for an offer this account sent and learns later
   was accepted: no snapshot from before the trade, no partner read.
   
   - looks once at Cs2::inventory: the new items are those whose id isn't in
     known_ids; nothing is accepted, nothing is waited for
   - fingerprints: the partner's copies' pattern and wear by their asset id
     in the offer, as the caller read them before the trade; any may be
     missing
   - places items with the rules of accept_offer_and_find_items: each item
     with a fingerprint on the closest new item that matches it, the same
     pattern and a wear within 1e-6 so that a wear rounded to 6 decimals
     still matches, the lowest id of equally close ones (Located::Exact),
     else NotFound; then each item without one on the first new item our
     web inventory lists with its kind (Located::AnyOfKind), else NotFound;
     kinds are read once, in context 16, only when an item has no
     fingerprint
   - known_ids should include the new ids earlier calls found for other
     trades: else two offers with the same commodity can be given the same
     new id
   - TradeError::Cs2InventoryUnavailable while the game coordinator's
     inventory isn't current (nothing looked up)
   - Fingerprint is public: Fingerprint::new(pattern, wear: f64) (the wear
     kept as f32, the game coordinator's float),
     Fingerprint::of(&InventoryItem), pattern(), wear() -> f32; serde as
     {"pattern": 302, "wear": 0.37215376}, the wear the shortest decimal
     that reads back as the same f32
   
   Not verified against live Steam.
 - <csr-id-8ad274fbaee8a5df823c5bbdc18580f8835fbc0b/> Inventory::fingerprints
   Inventory::fingerprints() (feature cs2): the Fingerprint of each item
   with a pattern and a wear, by its asset id, to pass to
   accept_offer_and_find_items or find_received_items after one read of
   the partner's inventory. Commodities have none.
 - <csr-id-832c3617f05a144c63cebfc15445b7082064f6f3/> Inventory::total_inventory_count
   The count Steam sends on every inventory page, to tell a complete read:
   Inventory::total_inventory_count() (the first page's) and
   InventoryPage::total_inventory_count(). None when Steam doesn't send it,
   or not as a count.

### Bug Fixes

 - <csr-id-cf9b96a4d8e0ee12e42f79171d9f14dd794c2e79/> find_received_items places items without a fingerprint by kind only
   The new items may have come another way, so an item without a
   fingerprint (every commodity) is placed only on a new item our web
   inventory lists with its classid (Located::AnyOfKind), else
   Located::NotFound.

 - <csr-id-f2b1b4d5a46d2d3b2204c5eac4c75bcd66c44d49/> find_received_items fails with a web inventory read that failed
   A failed read of our web inventory for the new items' kinds is the call's
   error (the read's TradeError; is_too_many_requests() tells a 429) instead
   of a logged Located::NotFound.

### Refactor

 - <csr-id-e543c5532288e1700f8dd71af80f1685275ccc37/> one placing core for received items
   accept_offer_and_find_items and find_received_items share it.

## v0.7.0 (2026-10-21)

### Chore

 - <csr-id-8cba64f4e70cd363e2fb272f9fe16df5bb95c147/> remove unused dependencies

### Documentation

 - <csr-id-dbaedcc06760a687b4d5bb32e578fe3235f98a9e/> update READMEs

### New Features

 - <csr-id-885079f00dde917da014a06bd7b25750f6500eaf/> where an accepted offer's items went (feature cs2)
   Since 2026-04 Steam's trade history has no new_assetid for CS2 trades.
   SteamTradeManager::accept_offer_and_find_items(offer_id, &mut Cs2)
   accepts as accept_offer, then finds where each received item went
   through CS2's game coordinator.
   
   - feature cs2 depends on steam-client-lib's cs2 and turns on
     steam-mobile's cm, so that one Steam client logon gives both the CS2
     session and the web session (login_with_client)
   - returns AcceptedOffer { tradeid, received }, each ReceivedItem
     { offered, located } with Located::{Only, PatternAndWear, Kind,
     Guessed, NotFound} and the new asset id
   - waits for the new ids by looking at Cs2::inventory every 250 ms (no
     request, the caller's events stay), 60 s by default
     (with_received_items_timeout)
   - web reads: none for one item, the partner's inventory (once per
     context) only when 2+ received items share a classid, our own context
     16 (1 to 3 reads) only when the game coordinator can't tell kinds
     apart
   - fails before accepting with TradeError::Cs2InventoryUnavailable when
     the game coordinator's inventory isn't current; after the accept, a
     failure is TradeError::AcceptedButItemsNotFound with the trade id;
     also new: TradeError::GameCoordinatorError
   - holds the manager's accept lock until the items are found
   - examples full_trade (a whole CS2 trade between two accounts, asks
     before sending and accepting) and cs2_trade_items
   
   Not verified against live Steam.

### Refactor

 - <csr-id-d412496028f2434a1914da0a83029bdbcbb6b915/> use steam-language enums in tappet, steam-trading and steam-mobile
   These crates only used EResult, ETradeOfferState and
   ETradeOfferConfirmationMethod, which are the same types steam-language-gen
   re-exports. They now use steam-language without features, so tappet no
   longer builds steam-protobuf at all; steam-mobile still does for its own
   protobuf messages.

### New Features (BREAKING)

 - <csr-id-677fbe595d3e9dbb0566e88123ec95349dbb3243/> access token, trade protection, inventories
   Through tappet, the Web API calls (GetTradeOffers, GetTradeHistory) go
   to https://api.steampowered.com and send the session's access token
   (access_token=) instead of the API key (key=), with the rest of the
   query unchanged. With an API key, Steam leaves CS2 offers and trades
   out (seen live 2026-10-20); the token sees them, as in
   node-steam-tradeoffer-manager by default. Verified live for reading
   offers and the history.
   
   - trade protection is acknowledged automatically: before a manager's
     first confirmation (in create_offer_and_confirm, and in accept_offer
     when Steam asks for one) it sends POST /trade/new/acknowledge once, as
     ArchiSteamFarm does. Steam refuses to confirm CS2 trades until the
     account acknowledged (seen live 2026-10-20). A failed acknowledgement
     is logged, the confirmation still tried, and the next confirmation
     sends it again. acknowledge_trade_protection sends it by hand. When
     it sends, before the first confirmation, is not verified live
   - get_trade_offers (and so get_tradeoffer_by_id, accept_offer,
     decline_received_offers) follows Steam's next_cursor and merges every
     page; the first request is unchanged, the next ones add &cursor=N.
     Like node-steam-tradeoffer-manager it returns every page or an error:
     a later page that fails fails the call, and a repeated cursor or more
     than 100 pages is TradeError::TradeOffersIncomplete
   - get_trade_offers fails with TradeError::MalformedOffers when Steam
     sent an offer tappet could not read, instead of failing to parse the
     whole response. get_tradeoffer_by_id fails only if the unreadable
     offer may be the requested one (MalformedOffer with its id); a
     readable copy wins. deny_offer / cancel_offer with the id from the
     error get past it
   - trade_lock_end: when a trade's items stop being trade protected, as
     Steam reports it (time_settlement in GetTradeHistory), None if Steam
     sends no time
   - InventoryContext names where items are kept, in place of an appid and
     contextid pair: CS2, CS2_TRADE_PROTECTED, DOTA2, TF2, STEAM, and
     InventoryContext::new(appid, contextid) for other games
   - inventory(steamid, context) and own_inventory(context): a Steam
     Community inventory, every page of it, as an Inventory whose pages()
     give InventoryItems (ids, name, tradable / marketable / commodity,
     owner-only texts such as the trade protection end, CS2's
     asset_properties with pattern() and wear()). Reading a page allocates
     nothing per item, reading items nothing at all (tests/zero_alloc.rs);
     InventoryPage::parse reads a body of your own. New request
     GET /inventory/{steamid}/{appid}/{contextid}?l=english&count=1000 (as
     sent live), then &start_assetid=... while Steam says more items come
     (not seen live)
   - trade_history(max_trades, include_failed): the GetTradeHistory call
     the crate already made, public
   - my_trade_link(): the account's own trade link, from
     IEconService/GetTradeOfferAccessToken (the service method
     ArchiSteamFarm calls over CM), else the account's
     /profiles/{id}/tradeoffers/privacy page (as node-steamcommunity and
     aiosteampy read it). Not verified live
   - accept_offer returns the trade id from Steam's answer to the accept
     ({"tradeid": "..."}, seen live), None when Steam asked for a mobile
     confirmation
   - HasAssets for GetTradeOffersResponse and FilterBy's partition_by are
     implemented (they were unimplemented!())
   - SteamID is re-exported

### Bug Fixes (BREAKING)

 - <csr-id-eca51fe934e9ef9bcca5443d248365009e3e6d7f/> read Steam's answers without panics
   Every existing request sends exactly what it sent before (URL, form,
   Referer); only how Steam's answers are read changed.
   
   - a response with a strError is an error (with the EResult in its (N),
     else OfferError::GeneralFailure); it used to parse as a success for
     decline and cancel, and to panic on create
   - HTTP 429 from Steam Community is steam-mobile's
     InternalError::TooManyRequests, any other non-2xx without a strError
     InternalError::HttpStatus, both under TradeError::InternalError;
     TradeError::is_too_many_requests(), also true for a 429 to fetching
     or accepting confirmations
   - no more panics: create_offer without a trade offer id in Steam's
     answer, accept_offer without needs_mobile_confirmation,
     get_new_assetids for a trade not in the history, a strError without a
     known code, Tradelink::validate_with_steam64 with a partner above
     u32::MAX
   - accept_offer confirms only when Steam answers
     needs_mobile_confirmation: true. Without either flag the accept is
     done, as node-steam-tradeoffer-manager reads it (it failed with
     ConfirmationError::NotFound after a successful accept, e.g. of a
     gift). needs_email_confirmation: true alone is the new
     TradeError::NeedsEmailConfirmation instead of a success
   - create_offer_and_confirm waits the intended second between creating
     and confirming (the delay was never awaited), and once the offer
     exists every error carries its id
   - decline_received_offers declines one offer at a time, a second apart
     (they all ran at once), and reports every result
   - trade links must start the string, ASCII only: a link merely
     containing a trade link was accepted (e.g. as a parameter of another
     site's link), and so were Unicode letters and digits. Surrounding
     whitespace, and more parameters or a fragment after the token, are
     accepted; a second partner or token parameter is rejected, and so are
     percent-encoded parameter names and partner[] / token[]
   - network errors (InternalError::NetworkError) show their URL without
     its query
   - TradeOffer::validate accepts exactly 255 items per side, and rejects
     empty item collections on both sides
   - check_steam_guard_recently_activated needs the additional-checks
     feature (the default); the crate builds without default features

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release over the course of 10 calendar days.
 - 987 days passed between releases.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Where an accepted offer's items went (feature cs2) ([`885079f`](https://github.com/saskenuba/SteamHelper-rs/commit/885079f00dde917da014a06bd7b25750f6500eaf))
    - Access token, trade protection, inventories ([`677fbe5`](https://github.com/saskenuba/SteamHelper-rs/commit/677fbe595d3e9dbb0566e88123ec95349dbb3243))
    - Read Steam's answers without panics ([`eca51fe`](https://github.com/saskenuba/SteamHelper-rs/commit/eca51fe934e9ef9bcca5443d248365009e3e6d7f))
    - Remove unused dependencies ([`8cba64f`](https://github.com/saskenuba/SteamHelper-rs/commit/8cba64f4e70cd363e2fb272f9fe16df5bb95c147))
    - Update READMEs ([`dbaedcc`](https://github.com/saskenuba/SteamHelper-rs/commit/dbaedcc06760a687b4d5bb32e578fe3235f98a9e))
    - Use steam-language enums in tappet, steam-trading and steam-mobile ([`d412496`](https://github.com/saskenuba/SteamHelper-rs/commit/d412496028f2434a1914da0a83029bdbcbb6b915))
</details>

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

