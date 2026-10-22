# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.8.0 (2026-10-22)

### New Features (BREAKING)

 - <csr-id-c8c72681989e61d0fd40bf423db8a6f040672e22/> GetTradeOffer as Steam answers it
   - GetTradeOffer takes the offer id as u64, like GetTradeOffers' ids.
   - CEcon_GetTradeOffer_Response is { offer: Option<TradeOffer_Trade>,
     malformed_offer: Option<MalformedOffer> }: Steam answers a missing
     offer with {"response":{}} and x-eresult 42 (seen live 2026-10-22),
     and an offer that can't be read is kept aside, as in GetTradeOffers.
   - No language or get_descriptions in the query when they are None.
   - TradeOffer_Trade::settlement_date documented: seen live, the trade's
     time_settlement once accepted, 0 before.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 1 commit contributed to the release.
 - 1 day passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - GetTradeOffer as Steam answers it ([`c8c7268`](https://github.com/saskenuba/SteamHelper-rs/commit/c8c72681989e61d0fd40bf423db8a6f040672e22))
</details>

## 0.7.0 (2026-10-21)

### New Features (BREAKING)

 - <csr-id-4438588a450e69a40f1d2d352e6ccbe21040cd15/> lenient trade types, offer paging, GetTradeOffersSummary
   Trade responses no longer fail as a whole when Steam leaves out a field
   its schema marks optional, sends an enum value this version doesn't
   know, or sends an item without appid, contextid or assetid.
   
   - GetTradeOffers(..).cursor(n) and
     CEcon_GetTradeOffers_Response::next_page_cursor(): Steam pages
     GetTradeOffers, so far only the first page was read
   - GetTradeOffer(..).get_descriptions(bool)
   - IEconService/GetTradeOffersSummary, with
     GetTradeOffersSummaryResponse
   - an offer GetTradeOffers can't read (e.g. without accountid_other,
     tradeofferid or trade_offer_state) no longer fails the response: it
     is kept as a MalformedOffer (raw JSON, reason, tradeofferid()) in
     malformed_offers_sent / malformed_offers_received, see
     has_malformed_offers()
   - TradeOffer_Trade exposes expiration_time, items_to_give,
     items_to_receive, from_real_time_trade and confirmation_method, which
     were private; has_malformed_items(), malformed_items(),
     assets_to_give() and assets_to_receive(), with MalformedItems
     (NoItems | Item) as the reason. An offer with no item on either side
     is malformed, as node-steam-tradeoffer-manager counts it; a gift
     (items on one side only) is not. As in node-steam-tradeoffer-manager,
     if Steam leaves out only one of the two lists, the offer reads as a
     gift
   - trade protection fields TradeOffer_Trade::{eresult,
     partner_confirmation_method, delay_settlement, settlement_date}
   - ETradeOfferState, ETradeOfferConfirmationMethod and EResult compare
     equal to an EnumOrUnknown of themselves, both ways
   - ETradeOfferConfirmationMethod is re-exported from response_types
   
   What still fails a whole response: a field of the response itself that
   can't be read (e.g. a next_cursor that is not a number); in
   GetTradeHistory, one trade without tradeid, steamid_other or status, or
   one traded asset or currency without its ids; in GetTradeOffer, an
   offer that can't be read.
 - <csr-id-696343660a23980bad10ba4ccfdd50c093ae2c79/> access tokens, https, responses checked before parsing
   Every call goes to https://api.steampowered.com (was http://), with an
   API key or an access token, since the query carries the credential, as
   node-steam-tradeoffer-manager sends it. With an API key every existing
   call sends the same request as before except for the scheme: query (and
   its order, with the key=...&& double ampersand), form and headers.
   
   - Credential (ApiKey | AccessToken); SteamAPI::with_access_token,
     SteamAPI::with_client(client, credential), which reuses an HTTP
     client for a SteamAPI per call, and SteamAPI::set_credential. An
     access token is sent as access_token= instead of key=, in the query
     and in POST forms; inject_custom_key still wins over it. SteamAPI and
     Credential never show the secret in Debug
   - SteamAPIError::HttpError holds the reqwest::Error without its URL's
     query, so its Display and Debug never show key=, access_token= or a
     partner's trade_offer_access_token=
   - execute_with_response checks the HTTP status, x-eresult and
     X-error_message before parsing, like execute: a failed call no longer
     parses as an empty response
   - both executors: x-eresult 2 with a non-empty body is a success (Steam
     sends it for calls that worked; node-steam-tradeoffer-manager's
     rule); a non-200 status is an error even with x-eresult 1
   - no more panics on a non-ASCII or unknown x-eresult / X-error_message,
     or on a body that doesn't decode (blocking execute_with_response)
   - the blocking feature compiles again
   - serde_with requirement raised to ^1.5, the first with serde_as

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 3 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Lenient trade types, offer paging, GetTradeOffersSummary ([`4438588`](https://github.com/saskenuba/SteamHelper-rs/commit/4438588a450e69a40f1d2d352e6ccbe21040cd15))
    - Access tokens, https, responses checked before parsing ([`6963436`](https://github.com/saskenuba/SteamHelper-rs/commit/696343660a23980bad10ba4ccfdd50c093ae2c79))
</details>

## 0.6.1 (2026-10-18)

### Refactor

 - <csr-id-d412496028f2434a1914da0a83029bdbcbb6b915/> use steam-language enums in tappet, steam-trading and steam-mobile
   These crates only used EResult, ETradeOfferState and
   ETradeOfferConfirmationMethod, which are the same types steam-language-gen
   re-exports. They now use steam-language without features, so tappet no
   longer builds steam-protobuf at all; steam-mobile still does for its own
   protobuf messages.

### Documentation

 - <csr-id-dbaedcc06760a687b4d5bb32e578fe3235f98a9e/> update READMEs

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release over the course of 7 calendar days.
 - 984 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Update READMEs ([`dbaedcc`](https://github.com/saskenuba/SteamHelper-rs/commit/dbaedcc06760a687b4d5bb32e578fe3235f98a9e))
    - Use steam-language enums in tappet, steam-trading and steam-mobile ([`d412496`](https://github.com/saskenuba/SteamHelper-rs/commit/d412496028f2434a1914da0a83029bdbcbb6b915))
</details>

## 0.6.0 (2024-02-07)

### Chore

 - <csr-id-c25d9961676b175df62775dc49bf7723c72fac00/> updated tappet changelog

### New Features

 - <csr-id-8490b470ab165e064bf7f6cced4031cb4437b082/> made time_created and time_updated public on tradeoffer response endpoint

### Other

 - <csr-id-7060a0ef865a3ddc1b45aa0276e112b15b9f7773/> bump to 0.6

### New Features (BREAKING)

 - <csr-id-9b29933a67be14d210a0cb28df9de2a04ba5a161/> steam-trading is compiling again.
   - Improved docs;
   - Clippy lints
   
   BREAKING CHANGES:
   
   - All tradeoffer_ids are now u64
   - SteamAPI client is non lazy now. It avoids Sync issues now, and it would error
   anyway if API key wasn't cached.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 9 commits contributed to the release.
 - 607 days passed between releases.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Bump to 0.6 ([`7060a0e`](https://github.com/saskenuba/SteamHelper-rs/commit/7060a0ef865a3ddc1b45aa0276e112b15b9f7773))
    - Updated tappet changelog ([`c25d996`](https://github.com/saskenuba/SteamHelper-rs/commit/c25d9961676b175df62775dc49bf7723c72fac00))
    - Made time_created and time_updated public on tradeoffer response endpoint ([`8490b47`](https://github.com/saskenuba/SteamHelper-rs/commit/8490b470ab165e064bf7f6cced4031cb4437b082))
    - Steam-trading is compiling again. ([`9b29933`](https://github.com/saskenuba/SteamHelper-rs/commit/9b29933a67be14d210a0cb28df9de2a04ba5a161))
    - Merge pull request #14 from Bash-09/master ([`141ed8f`](https://github.com/saskenuba/SteamHelper-rs/commit/141ed8fc443530ab3fee46399a3c16081a148d10))
    - Some reponses don't have personastateflags ([`3b1e462`](https://github.com/saskenuba/SteamHelper-rs/commit/3b1e462b9a22afcd2acc26edb7ddac1cc35e2f86))
    - Add profilehash to PlayerSummary ([`f44cccb`](https://github.com/saskenuba/SteamHelper-rs/commit/f44cccbc5cf85fc28e5698b11ac4cc4a3912b057))
    - FriendList can be None ([`ccf2e00`](https://github.com/saskenuba/SteamHelper-rs/commit/ccf2e00442cdbe5fdd9085816d1c1a1ff1c932f3))
    - Add GetFriendList response ([`fdb5314`](https://github.com/saskenuba/SteamHelper-rs/commit/fdb53148264f7f7b5461299f51611d39b38f74b3))
</details>

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

