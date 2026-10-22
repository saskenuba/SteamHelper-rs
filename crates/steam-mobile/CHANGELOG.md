# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.7.0 (2026-10-21)

### Chore

 - <csr-id-8cba64f4e70cd363e2fb272f9fe16df5bb95c147/> remove unused dependencies

### Documentation

 - <csr-id-dbaedcc06760a687b4d5bb32e578fe3235f98a9e/> update READMEs

### New Features

 - <csr-id-40dcb71581b1b3181cafd67898205cb0ecc76bc8/> log in through a Steam client (feature cm)
   Feature cm, an optional dependency on steam-client-lib:
   SteamAuthenticator::login_with_client(SteamHandle<User>) logs in to the
   Steam websites through a Steam client logged on to the account, instead
   of with a password. The client generates an access token
   (Authentication.GenerateAccessTokenForApp over its CM connection); the
   session sets the steamLoginSecure and sessionid cookies on the
   Community, Store, Help and Checkout hosts, and checks that Steam takes
   them.
   
   The authenticator keeps the handle and renews the token itself: the
   first request with the session (or access_token()) once the current
   token expires within an hour first gets a new one through the client,
   swaps steamLoginSecure and keeps the sessionid.
   
   - SteamAuthenticator::renew_before(Duration) sets another margin, which
     is also how long the session outlasts a client that can't renew it
   - the token's lifetime is read from its own claims (exp minus iat, or
     exp by the local clock without iat) and counted from when the client
     was asked, so a skewed local clock doesn't matter
   - the request that starts a renewal waits for the client's answer, up
     to its 30-second call deadline (steam-mobile spawns no task);
     requests meanwhile go at once with the current token while it is
     valid, and only those whose token has expired wait for that answer,
     so the client is asked once
   - a failed renewal (such as while the client reconnects) lets the
     request go with the current token while it is valid, and isn't tried
     again for 30 seconds. An expired token is never sent: such a request
     fails, unsent, with InternalError::ClientToken, whose source is the
     client's error as an Arc<steam_client::errors::ClientError>
   - a valid token issued for less than an hour is renewed 30 seconds
     after it was asked for at the soonest, not on every request; a token
     whose expiry is out of range is an error, not a panic
   - renewal happens only early, and a failed request is never retried.
     Password logins never renew, as before. The mid-session swap is not
     verified against live Steam
   - SteamAuthenticator::access_token().await, returning an AccessToken
     whose Debug hides the token, for Web API calls that take
     access_token= (steam-trading's SteamTradeManager). Async and fallible
     because a login through a Steam client renews the token first when it
     is about to expire
   - SteamAuthenticator::steam_id() (logged in): the account's SteamID,
     from the login
   - StatusCode is re-exported
   - examples web_session (step by step with a Steam client, read-only) and web_session_bot (the long-running setup), feature cm;
     steam-client-lib's examples point to them
 - <csr-id-5b53d00168dffc3ed658a44bc4e3e7a70ab70520/> revoke the API key, so it can be reissued
   - SteamAuthenticator::revoke_api_key posts to /dev/revokekey and clears
     the cached key; a following register_api_key registers a fresh one
   - the revocation is verified by re-fetching the key page, since Steam
     does not confirm it in the response; revoking when no key exists
     succeeds
   - examples: login, and rotate_api_key, which revokes and registers a new
     key end to end

### Bug Fixes

 - <csr-id-358fd47498708ff62993662676bb93bdb85efc97/> parse family group and unknown confirmation kinds
   EConfirmationType now mirrors Steam's EMobileConfirmationType, adding
   BuildChangeRequest, AddUser, InviteToFamilyGroup, JoinFamilyGroup,
   MarketPurchase and RequestRefund. Kinds Steam adds later deserialize as
   Unknown instead of failing the whole confirmation fetch.

### New Features (BREAKING)

 - <csr-id-eff8fe79116bf58b51c9c1e0dd26acd9ce6cda1b/> add authenticators without a phone
   Follows ArchiSteamFarm and steamguard-cli: AddAuthenticator is called
   directly, and Steam sends the activation code by SMS if the account has
   a phone, or by email otherwise.
   
   - new PendingMaFile state. SteamAuthenticator<Authenticated, AbsentMaFile>
     ::begin_add_authenticator moves to it and reports where the code went
     (ActivationCodeDelivery, from confirm_type and phone_number_hint).
     finalize_pending moves it to PresentMaFile, ready for confirmations, or
     hands it back with the error so a wrong code can be retried in the
     same session
   - SteamUser::with_pending_mafile resumes a pending maFile from disk; login
     doesn't use it for codes, since it isn't active yet
   - compile_fail doctests check that these transitions can't be skipped
   - AddAuthenticator failures say what happened (rate limited, refused,
     the raw status) instead of "Something went wrong"
   - CLI: --phone is optional for authenticator add. With --out, the maFile
     is written as <account>.maFile.pending before finalizing and renamed to
     <account>.maFile afterwards, never overwriting an existing file. A
     wrong code is asked again, and authenticator finalize resumes from the
     pending file
   - README documents the new flow
 - <csr-id-ee0f9b2081487dac8ef3bafb233b7b1f83c6dc24/> recover missing device IDs, let the maFile state drive login
   - a sealed MaFileState trait, implemented by AbsentMaFile and
     PresentMaFile, tells login which maFile is active. Login takes the
     shared secret from it instead of downcasting an Arc<dyn IsUser>;
     IsUser and downcast-rs are gone, and an invalid shared secret is a
     login error instead of a panic
   - when the active maFile has no device ID, login fetches it from
     ITwoFactorService/QueryStatus, and fails if Steam has none. An
     authenticated PresentMaFile therefore always has one, so confirmations
     and API key registration no longer panic on maFiles that lack it
   - MobileAuthFile::device_id and SteamAuthenticator::mafile expose it, so
     the recovered ID can be saved back
   - CLI: authenticator status shows the Steam Guard status, compares the
     device IDs, and offers to save a recovered one, replacing the maFile
     through a temporary file
 - <csr-id-20c78b5cba297ba8e57ba67e8b95ecb480df3b1f/> revamp the CLI
   - clap 4 derive; the binary is now steam-mobile and reports the crate
     version
   - the maFile comes from --mafile or STEAM_MAFILE, and the account from
     the maFile, overridable with --account or STEAM_ACCOUNT
   - code prints the login code from the maFile's shared secret, through
     the new MobileAuthFile::auth_code
   - confirmations list shows the pending confirmations; accept and deny
     take ids, --trade-offer, --kind (repeatable, `family` covers invites
     and joins) or --all, and print what they act on. Unknown ids fail
     before anything is sent
   - authenticator add takes --phone and --out
   - README documents the new commands

### Bug Fixes (BREAKING)

 - <csr-id-df9e30d0abb695141efec20c5fa6aee2f849141d/> own cookie jar, responses checked before parsing
   Requests are unchanged, except that a missing sessionid cookie now stops
   some of them (see dump_cookie below); only how Steam's answers are read
   changed.
   
   - the cookie jar no longer loses cookies at random. The
     cookie-hashed-domain jar compared cookies by name only, so now and
     then a sessionid or steamLoginSecure of one Steam host replaced the
     one of another (about 9% of sessions lost one; steam-trading then
     sent an empty sessionid). steam-mobile has its own jar now, keyed by
     host and name, and depends on upstream cookie 0.17 for the cookie
     type only. Requests send the same cookies as before; only their order
     in the Cookie header changed, from random to fixed (the mobile app's
     cookies first, then the others in the order they were first set)
   - responses parsed as a success are checked first: HTTP 429 is
     InternalError::TooManyRequests, any other non-2xx
     InternalError::HttpStatus (with the start of the page), and a Web API
     x-eresult other than OK InternalError::EResult (an InvalidPassword at
     login is LoginError::IncorrectCredentials). They were parsed anyway
     and failed as deserialization errors
   - a protobuf response that doesn't decode says so, with the type and
     the decoder's error
   - an EResult number steam-language doesn't know no longer fails the API
     key and parental unlock responses; error messages show the number
   - dump_cookie returns None when the session has no such cookie (it
     returned Some("") when the jar held other cookies). The login steps
     that send sessionid still send an empty one when there is none,
     without panicking. api_key_register and api_key_revoke (and so
     steam-trading's Steam Community POSTs) instead fail without sending
     anything, with ApiKeyError::GeneralError("Missing sessionid
     cookie."), where they used to POST an empty sessionid
   - InternalError::HttpError (and every error built from it) shows its
     URL without the query, which may carry a confirmation hash or an
     encoded request

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 10 commits contributed to the release over the course of 9 calendar days.
 - 10 days passed between releases.
 - 9 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Log in through a Steam client (feature cm) ([`40dcb71`](https://github.com/saskenuba/SteamHelper-rs/commit/40dcb71581b1b3181cafd67898205cb0ecc76bc8))
    - Own cookie jar, responses checked before parsing ([`df9e30d`](https://github.com/saskenuba/SteamHelper-rs/commit/df9e30d0abb695141efec20c5fa6aee2f849141d))
    - Release steam-cryptography v0.1.0, steam-language-gen-derive v0.2.1, steam-protobuf v0.3.1, steam-totp v0.2.3, tappet v0.6.1, steam-client-lib v0.1.0 ([`6c17375`](https://github.com/saskenuba/SteamHelper-rs/commit/6c173751b28bf95bfe2983dcaf48cf103e9a4307))
    - Remove unused dependencies ([`8cba64f`](https://github.com/saskenuba/SteamHelper-rs/commit/8cba64f4e70cd363e2fb272f9fe16df5bb95c147))
    - Update READMEs ([`dbaedcc`](https://github.com/saskenuba/SteamHelper-rs/commit/dbaedcc06760a687b4d5bb32e578fe3235f98a9e))
    - Add authenticators without a phone ([`eff8fe7`](https://github.com/saskenuba/SteamHelper-rs/commit/eff8fe79116bf58b51c9c1e0dd26acd9ce6cda1b))
    - Recover missing device IDs, let the maFile state drive login ([`ee0f9b2`](https://github.com/saskenuba/SteamHelper-rs/commit/ee0f9b2081487dac8ef3bafb233b7b1f83c6dc24))
    - Revamp the CLI ([`20c78b5`](https://github.com/saskenuba/SteamHelper-rs/commit/20c78b5cba297ba8e57ba67e8b95ecb480df3b1f))
    - Parse family group and unknown confirmation kinds ([`358fd47`](https://github.com/saskenuba/SteamHelper-rs/commit/358fd47498708ff62993662676bb93bdb85efc97))
    - Revoke the API key, so it can be reissued ([`5b53d00`](https://github.com/saskenuba/SteamHelper-rs/commit/5b53d00168dffc3ed658a44bc4e3e7a70ab70520))
</details>

## 0.6.0 (2026-10-11)

### Other

 - <csr-id-75ec4dcf47f0d9bbbc07868692183bd62bd55377/> steamid-parser 1.0

### Refactor

 - <csr-id-d412496028f2434a1914da0a83029bdbcbb6b915/> use steam-language enums in tappet, steam-trading and steam-mobile
   These crates only used EResult, ETradeOfferState and
   ETradeOfferConfirmationMethod, which are the same types steam-language-gen
   re-exports. They now use steam-language without features, so tappet no
   longer builds steam-protobuf at all; steam-mobile still does for its own
   protobuf messages.

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

### Bug Fixes (BREAKING)

 - <csr-id-95d3d86fdd49339ddbac77a5f8240cd12f546cea/> remove the cache lock, check Steam's answers, stop logging secrets
   - the session cache is no longer behind a parking_lot RwLock held across
     awaits, so every public future is Send; a compile-time test checks it
   - finalize gives up after 30 tries instead of spinning on status 88, and
     checks success; wrong SMS codes, rejected phone numbers, unconfirmed
     emails and failed removals now return errors (new LinkerError variants)
   - finalize_authenticator now awaits the phone catch-up delay
   - remove_authenticator sends the access token as a query parameter;
     RemoveAuthenticatorScheme and QueryStatusResponse are exported
   - login no longer registers an API key and accepts every pending API key
     confirmation; register_api_key does it explicitly and accepts only the
     confirmation for its own request
   - logs and Debug output no longer include tokens, cookies, request or
     response bodies, or the AddAuthenticator secrets
   - MobileAuthFile::from_disk returns an error instead of panicking
   - the crate builds again on current rustc (lints are warn, not deny), and
     stale tests are fixed or removed
   - CLI: auth add no longer panics; confirmation respects -i and -a and
     exits non-zero on failure; the password comes from STEAM_PASSWORD or a
     prompt; -p is a directory and an existing maFile is never overwritten;
     the binary requires the cli feature

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 977 days passed between releases.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Remove the cache lock, check Steam's answers, stop logging secrets ([`95d3d86`](https://github.com/saskenuba/SteamHelper-rs/commit/95d3d86fdd49339ddbac77a5f8240cd12f546cea))
    - Regenerate with lite runtime, one feature per proto ([`cd7631a`](https://github.com/saskenuba/SteamHelper-rs/commit/cd7631af6c19be5790615e75a5953192d201549f))
    - Use steam-language enums in tappet, steam-trading and steam-mobile ([`d412496`](https://github.com/saskenuba/SteamHelper-rs/commit/d412496028f2434a1914da0a83029bdbcbb6b915))
    - Steamid-parser 1.0 ([`75ec4dc`](https://github.com/saskenuba/SteamHelper-rs/commit/75ec4dcf47f0d9bbbc07868692183bd62bd55377))
</details>

## 0.5.0 (2024-02-07)

### Documentation

 - <csr-id-0142d141bebfbe9d641ef98098ee9f9c8acd1757/> fixed wrong docs

### New Features

 - <csr-id-fe6c19345b15cb4b63f75b05418e0213e8b2b665/> cli is compiling again
 - <csr-id-8fea774279551db86f6b92315a059ab5339ced9b/> adjusted confirmation inner methods
 - <csr-id-c3b7298bf8593a494e51078bb7b4b31582783835/> registration of new api key is now working again; sending confirmations is working again
 - <csr-id-485daa67302716dbdd7c3de61722db4adda34727/> fetch of login RSA key is now protobuf backed
 - <csr-id-db6e83a347b6aa3df4f0e1811cd354861839934d/> User is now Steam<T>, where T is the user having a mafile or not.
   this allows methods that requires a mafile to not show up on completions
   
   * SteamUser states are SteamUser<PresentMaFile> and SteamUser<AbsentMaFile>;
   * Confirmation retrieval is now working correctly;
   * API Key registration is almost working again, only sending confirmations are
   needed;
   * Removed manually adding cookies after pinging steam domains, this is done
   automatically now by request method;
 - <csr-id-9d864866b00e05bcf1cb1b7db389b5a2a4c11557/> cookies are handled directly by request, no need to manually input them on storage
 - <csr-id-65121ed55a3eb6d6b3068cc7d6ffe0bf6dc74c06/> removed unused confirmations scrapers, confirmations are now a json response
 - <csr-id-42998bd67bd0fc0bdae73db9b191ebc3461fb551/> removed unused types, added new confirmations kinds, adjusted Confirmation
 - <csr-id-8cf954b4d03e29891ea37f219c086b025faab05d/> removed warnings, fixed docs
 - <csr-id-95568ed52550c821212862ad0515e3cf2a69b6f4/> clippy lints, docs, renamed internal functions
 - <csr-id-897479ea62281f23a222bf735ceda6a22c557046/> login and storage of cookies working correctly, cleannup of docs and comments
 - <csr-id-02f302c9e67aebab3a7c9a892100b12fd5537de0/> all improved internal APIs returning only errors from own crate
 - <csr-id-fb218a3bdb7a047050c307b47c29800e51c59608/> new steam login method, removed unused old types
 - <csr-id-cad5810fed6cc3298a9497cb367ca7ebbb113d96/> added remove_authenticator method, clippy lints and fixes
 - <csr-id-d88d9c3cd338c670db487bda8482ebb33ddb76b6/> Remove Rc<RefCell<T>> from cookie jar and mobile client; organized cargo.toml

### Bug Fixes

 - <csr-id-4755b37cc75f3db54814e4df7c2e033287f713df/> exposed reqwest::Error, made MaFileState Send + Sync
 - <csr-id-d7799dd1f1eea85e0af06f50b4b791bfe0697b9c/> less debug! calls on proto request
 - <csr-id-996e28a7930f76adc8dd95ff90052557540e027d/> fixed STEAM_COMMUNITY_HOST
 - <csr-id-6505161011e05dad07450761006aae803858458d/> fixed an issue where request cookies weren't being passed correctly
 - <csr-id-7bdf9dff27171abb268cb7982c606f76c42b0267/> cleanup inner requests with new request_and_decode functions
 - <csr-id-d4323529730939b9dffa5af1aa0768591d414577/> clippy lints, renamed dump_cookies_by_name -> dump_cookies_by_domain_and_name

### Other

 - <csr-id-e4ffa07d827fea100c18420ed1c71e412b7f2d21/> steam-mobile 0.5
 - <csr-id-50d102bafa0ef94affbe94b896f22eb4167eb732/> made parking_lot available on workspace deps
 - <csr-id-dd51f9fda6bcdaccb3f6baeff70bf03fd325e3c4/> added changelog and bumped to 0.4.0
 - <csr-id-154212b00831bebc6ff4b7c351fee7c78dc90aa2/> removed num-derive, num-traits and added serde_repr from workspace
 - <csr-id-f37078a6c55650561b60faa6e9daac0b04efad41/> added some common used deps
 - <csr-id-179492088b413f18b5a6ea83167a2d5c807f58cc/> fixed clippy warnings
 - <csr-id-6457ec23fabec9fe7965b93eabea3ca4b850fe33/> added InternalError::GeneralFailure; set_steamid returns Result<T, InternalError>
 - <csr-id-54af8c01c71bbc511eae6d37e537b465842fe226/> typos and clippy lints on app cli
 - <csr-id-af9b9350dcefdf5e74e71fa890a365ac508571c4/> moved shared dependencies into workspace, added them to steam-mobile

### New Features (BREAKING)

 - <csr-id-d043dd67293431c9a81ee7c5f5b4e02e955c32e8/> Auth is now typed with SteamAuthenticator<Authenticated> with proper methods;
   * access_token is now working correctly for QueryStatus, for example, but still
   need to adapt all linker related methods;
   
   BREAKING CHANGES:
   * Removed weird wrapper around Confirmations;
   * Only method available on SteamAuthenticator<Unauthenticated> is login mostly;
 - <csr-id-757ff98ce1b619715ea076b4241e3252156e0757/> changes
   * BREAKING CHANGE: Added InternalError to return types;
   * BREAKING CHANGE: renamed MobileAuthFile from_str -> from_json;
   * Additional request and deserialize functions to avoid duplication;
   * CachedInfo is now SteamCache

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 39 commits contributed to the release over the course of 33 calendar days.
 - 595 days passed between releases.
 - 33 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Steam-mobile 0.5 ([`e4ffa07`](https://github.com/saskenuba/SteamHelper-rs/commit/e4ffa07d827fea100c18420ed1c71e412b7f2d21))
    - Exposed reqwest::Error, made MaFileState Send + Sync ([`4755b37`](https://github.com/saskenuba/SteamHelper-rs/commit/4755b37cc75f3db54814e4df7c2e033287f713df))
    - Made parking_lot available on workspace deps ([`50d102b`](https://github.com/saskenuba/SteamHelper-rs/commit/50d102bafa0ef94affbe94b896f22eb4167eb732))
    - Release steam-protobuf v0.2.0, steam-language-gen v0.1.3, steam-mobile v0.4.0 ([`89c10fa`](https://github.com/saskenuba/SteamHelper-rs/commit/89c10fa25974961bb4e90ad2167d94e95c3a8cf8))
    - Added changelog and bumped to 0.4.0 ([`dd51f9f`](https://github.com/saskenuba/SteamHelper-rs/commit/dd51f9fda6bcdaccb3f6baeff70bf03fd325e3c4))
    - Merge pull request #17 from saskenuba/mobile-confirmations-fix ([`bbec693`](https://github.com/saskenuba/SteamHelper-rs/commit/bbec69346043c586fcd3d8cdf2b04cc732f0b0d5))
    - Cli is compiling again ([`fe6c193`](https://github.com/saskenuba/SteamHelper-rs/commit/fe6c19345b15cb4b63f75b05418e0213e8b2b665))
    - Adjusted confirmation inner methods ([`8fea774`](https://github.com/saskenuba/SteamHelper-rs/commit/8fea774279551db86f6b92315a059ab5339ced9b))
    - Registration of new api key is now working again; sending confirmations is working again ([`c3b7298`](https://github.com/saskenuba/SteamHelper-rs/commit/c3b7298bf8593a494e51078bb7b4b31582783835))
    - Less debug! calls on proto request ([`d7799dd`](https://github.com/saskenuba/SteamHelper-rs/commit/d7799dd1f1eea85e0af06f50b4b791bfe0697b9c))
    - Fixed STEAM_COMMUNITY_HOST ([`996e28a`](https://github.com/saskenuba/SteamHelper-rs/commit/996e28a7930f76adc8dd95ff90052557540e027d))
    - Fetch of login RSA key is now protobuf backed ([`485daa6`](https://github.com/saskenuba/SteamHelper-rs/commit/485daa67302716dbdd7c3de61722db4adda34727))
    - User is now Steam<T>, where T is the user having a mafile or not. ([`db6e83a`](https://github.com/saskenuba/SteamHelper-rs/commit/db6e83a347b6aa3df4f0e1811cd354861839934d))
    - Cookies are handled directly by request, no need to manually input them on storage ([`9d86486`](https://github.com/saskenuba/SteamHelper-rs/commit/9d864866b00e05bcf1cb1b7db389b5a2a4c11557))
    - Removed num-derive, num-traits and added serde_repr from workspace ([`154212b`](https://github.com/saskenuba/SteamHelper-rs/commit/154212b00831bebc6ff4b7c351fee7c78dc90aa2))
    - Removed unused confirmations scrapers, confirmations are now a json response ([`65121ed`](https://github.com/saskenuba/SteamHelper-rs/commit/65121ed55a3eb6d6b3068cc7d6ffe0bf6dc74c06))
    - Removed unused types, added new confirmations kinds, adjusted Confirmation ([`42998bd`](https://github.com/saskenuba/SteamHelper-rs/commit/42998bd67bd0fc0bdae73db9b191ebc3461fb551))
    - Feat!(mobile): now all login is done through valid prototobufs ([`73862f1`](https://github.com/saskenuba/SteamHelper-rs/commit/73862f108903225537ab2c3a7eb489caeab2ad13))
    - Added some common used deps ([`f37078a`](https://github.com/saskenuba/SteamHelper-rs/commit/f37078a6c55650561b60faa6e9daac0b04efad41))
    - Merge pull request #16 from saskenuba/typed-mobile-refactor ([`4048602`](https://github.com/saskenuba/SteamHelper-rs/commit/4048602966d0a9981edc61253c7c74ba370c81b7))
    - Auth is now typed with SteamAuthenticator<Authenticated> with proper methods; ([`d043dd6`](https://github.com/saskenuba/SteamHelper-rs/commit/d043dd67293431c9a81ee7c5f5b4e02e955c32e8))
    - Merge pull request #15 from saskenuba/steam-mobile-login-revamp ([`d6cf2ef`](https://github.com/saskenuba/SteamHelper-rs/commit/d6cf2ef64b3efbd95dbd7d8de738c2a7d956ff2d))
    - Fixed clippy warnings ([`1794920`](https://github.com/saskenuba/SteamHelper-rs/commit/179492088b413f18b5a6ea83167a2d5c807f58cc))
    - Fixed wrong docs ([`0142d14`](https://github.com/saskenuba/SteamHelper-rs/commit/0142d141bebfbe9d641ef98098ee9f9c8acd1757))
    - Fixed an issue where request cookies weren't being passed correctly ([`6505161`](https://github.com/saskenuba/SteamHelper-rs/commit/6505161011e05dad07450761006aae803858458d))
    - Removed warnings, fixed docs ([`8cf954b`](https://github.com/saskenuba/SteamHelper-rs/commit/8cf954b4d03e29891ea37f219c086b025faab05d))
    - Clippy lints, docs, renamed internal functions ([`95568ed`](https://github.com/saskenuba/SteamHelper-rs/commit/95568ed52550c821212862ad0515e3cf2a69b6f4))
    - Login and storage of cookies working correctly, cleannup of docs and comments ([`897479e`](https://github.com/saskenuba/SteamHelper-rs/commit/897479ea62281f23a222bf735ceda6a22c557046))
    - Added InternalError::GeneralFailure; set_steamid returns Result<T, InternalError> ([`6457ec2`](https://github.com/saskenuba/SteamHelper-rs/commit/6457ec23fabec9fe7965b93eabea3ca4b850fe33))
    - Typos and clippy lints on app cli ([`54af8c0`](https://github.com/saskenuba/SteamHelper-rs/commit/54af8c01c71bbc511eae6d37e537b465842fe226))
    - Cleanup inner requests with new request_and_decode functions ([`7bdf9df`](https://github.com/saskenuba/SteamHelper-rs/commit/7bdf9dff27171abb268cb7982c606f76c42b0267))
    - Changes ([`757ff98`](https://github.com/saskenuba/SteamHelper-rs/commit/757ff98ce1b619715ea076b4241e3252156e0757))
    - All improved internal APIs returning only errors from own crate ([`02f302c`](https://github.com/saskenuba/SteamHelper-rs/commit/02f302c9e67aebab3a7c9a892100b12fd5537de0))
    - New steam login method, removed unused old types ([`fb218a3`](https://github.com/saskenuba/SteamHelper-rs/commit/fb218a3bdb7a047050c307b47c29800e51c59608))
    - Clippy lints, renamed dump_cookies_by_name -> dump_cookies_by_domain_and_name ([`d432352`](https://github.com/saskenuba/SteamHelper-rs/commit/d4323529730939b9dffa5af1aa0768591d414577))
    - Feat!(mobile): refactored Confirmations API to return Iterators on methods ([`2ae8d2b`](https://github.com/saskenuba/SteamHelper-rs/commit/2ae8d2bc84165d28b2b62348d006bae4b25d7f97))
    - Moved shared dependencies into workspace, added them to steam-mobile ([`af9b935`](https://github.com/saskenuba/SteamHelper-rs/commit/af9b9350dcefdf5e74e71fa890a365ac508571c4))
    - Added remove_authenticator method, clippy lints and fixes ([`cad5810`](https://github.com/saskenuba/SteamHelper-rs/commit/cad5810fed6cc3298a9497cb367ca7ebbb113d96))
    - Remove Rc<RefCell<T>> from cookie jar and mobile client; organized cargo.toml ([`d88d9c3`](https://github.com/saskenuba/SteamHelper-rs/commit/d88d9c3cd338c670db487bda8482ebb33ddb76b6))
</details>

## 0.4.0 (2024-01-23)

<csr-id-154212b00831bebc6ff4b7c351fee7c78dc90aa2/>
<csr-id-f37078a6c55650561b60faa6e9daac0b04efad41/>
<csr-id-179492088b413f18b5a6ea83167a2d5c807f58cc/>
<csr-id-6457ec23fabec9fe7965b93eabea3ca4b850fe33/>
<csr-id-54af8c01c71bbc511eae6d37e537b465842fe226/>
<csr-id-af9b9350dcefdf5e74e71fa890a365ac508571c4/>

### Documentation

 - <csr-id-0142d141bebfbe9d641ef98098ee9f9c8acd1757/> fixed wrong docs

### New Features

<csr-id-9d864866b00e05bcf1cb1b7db389b5a2a4c11557/>
<csr-id-65121ed55a3eb6d6b3068cc7d6ffe0bf6dc74c06/>
<csr-id-42998bd67bd0fc0bdae73db9b191ebc3461fb551/>
<csr-id-8cf954b4d03e29891ea37f219c086b025faab05d/>
<csr-id-95568ed52550c821212862ad0515e3cf2a69b6f4/>
<csr-id-897479ea62281f23a222bf735ceda6a22c557046/>
<csr-id-02f302c9e67aebab3a7c9a892100b12fd5537de0/>
<csr-id-fb218a3bdb7a047050c307b47c29800e51c59608/>
<csr-id-cad5810fed6cc3298a9497cb367ca7ebbb113d96/>
<csr-id-d88d9c3cd338c670db487bda8482ebb33ddb76b6/>

 - <csr-id-fe6c19345b15cb4b63f75b05418e0213e8b2b665/> cli is compiling again
 - <csr-id-8fea774279551db86f6b92315a059ab5339ced9b/> adjusted confirmation inner methods
 - <csr-id-c3b7298bf8593a494e51078bb7b4b31582783835/> registration of new api key is now working again; sending confirmations is working again
 - <csr-id-485daa67302716dbdd7c3de61722db4adda34727/> fetch of login RSA key is now protobuf backed
 - <csr-id-db6e83a347b6aa3df4f0e1811cd354861839934d/> User is now Steam<T>, where T is the user having a mafile or not.
   this allows methods that requires a mafile to not show up on completions
   
   * SteamUser states are SteamUser<PresentMaFile> and SteamUser<AbsentMaFile>;
* Confirmation retrieval is now working correctly;
* API Key registration is almost working again, only sending confirmations are
   needed;
* Removed manually adding cookies after pinging steam domains, this is done
   automatically now by request method;

### Bug Fixes

 - <csr-id-d7799dd1f1eea85e0af06f50b4b791bfe0697b9c/> less debug! calls on proto request
 - <csr-id-996e28a7930f76adc8dd95ff90052557540e027d/> fixed STEAM_COMMUNITY_HOST
 - <csr-id-6505161011e05dad07450761006aae803858458d/> fixed an issue where request cookies weren't being passed correctly
 - <csr-id-7bdf9dff27171abb268cb7982c606f76c42b0267/> cleanup inner requests with new request_and_decode functions
 - <csr-id-d4323529730939b9dffa5af1aa0768591d414577/> clippy lints, renamed dump_cookies_by_name -> dump_cookies_by_domain_and_name

### Other

 - <csr-id-154212b00831bebc6ff4b7c351fee7c78dc90aa2/> removed num-derive, num-traits and added serde_repr from workspace
 - <csr-id-f37078a6c55650561b60faa6e9daac0b04efad41/> added some common used deps
 - <csr-id-179492088b413f18b5a6ea83167a2d5c807f58cc/> fixed clippy warnings
 - <csr-id-6457ec23fabec9fe7965b93eabea3ca4b850fe33/> added InternalError::GeneralFailure; set_steamid returns Result<T, InternalError>
 - <csr-id-54af8c01c71bbc511eae6d37e537b465842fe226/> typos and clippy lints on app cli
 - <csr-id-af9b9350dcefdf5e74e71fa890a365ac508571c4/> moved shared dependencies into workspace, added them to steam-mobile

### Other

 - <csr-id-dd51f9fda6bcdaccb3f6baeff70bf03fd325e3c4/> added changelog and bumped to 0.4.0

### New Features (BREAKING)

<csr-id-757ff98ce1b619715ea076b4241e3252156e0757/>

 - <csr-id-d043dd67293431c9a81ee7c5f5b4e02e955c32e8/> Auth is now typed with SteamAuthenticator<Authenticated> with proper methods;
   * access_token is now working correctly for QueryStatus, for example, but still
   need to adapt all linker related methods;
* Removed weird wrapper around Confirmations;
* Only method available on SteamAuthenticator<Unauthenticated> is login mostly;
* BREAKING CHANGE: Added InternalError to return types;
* BREAKING CHANGE: renamed MobileAuthFile from_str -> from_json;
* Additional request and deserialize functions to avoid duplication;
* CachedInfo is now SteamCache

## 0.3.1 (2022-06-22)

<csr-id-e266af7ab0b709059f71c63e4e73eeea323fd1d4/>

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 36 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release steam-mobile v0.3.1 ([`0f532de`](https://github.com/saskenuba/SteamHelper-rs/commit/0f532dec00d961de6122295769aea6a5e9accd70))
    - Improved error handling of login and confirmations fetch ([`e266af7`](https://github.com/saskenuba/SteamHelper-rs/commit/e266af7ab0b709059f71c63e4e73eeea323fd1d4))
</details>

## 0.3.0 (2022-05-17)

<csr-id-0fc7ca6876a61d07945a4f6d5a0a937a44fe6af2/>
<csr-id-14404f4fd83c4c74893e3888693398d98bc3f199/>
<csr-id-5be4545d48846cf7e6ba166a545ce77fd451b26a/>
<csr-id-23f13a9e8927375f8a5dcd5be005e1c878132157/>

### New Features (BREAKING)

 - <csr-id-fdcf4076fe266964f5e8c9aa5beb81ab38281a51/> Added accept/deny mobile confirmations on CLI.
   * You can now accept and deny trade requests through the CLI;
* Bumped clap, uuid and dialoguer deps to latest versions;
* Refactored code of CLI to be more readable;

### Documentation

 - <csr-id-fb87360214c2f6d1319f467b82b27706ae157111/> added CHANGELOG.md, modified manifest versions
   We are now using cargo-smart-release to organize releases among with
   conventional commits;

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 11 commits contributed to the release over the course of 186 calendar days.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release steamid-parser v0.2.1, steam-mobile v0.3.0 ([`fd04253`](https://github.com/saskenuba/SteamHelper-rs/commit/fd0425344eb5d24093154320cc0ed81bf82a0b1a))
    - Release steamid-parser v0.2.1, steam-mobile v0.3.0 ([`ea7632d`](https://github.com/saskenuba/SteamHelper-rs/commit/ea7632d2fe5fcd85b48315f246f815afba88e62e))
    - Release steam-language-gen-derive v0.1.2, steam-protobuf v0.1.2, steam-language-gen v0.1.2, steam-totp v0.2.2, steamid-parser v0.2.1, steam-mobile v0.3.0 ([`cf773b0`](https://github.com/saskenuba/SteamHelper-rs/commit/cf773b07e0ae68376bf960d12f94ecb96afa9211))
    - Added CHANGELOG.md, modified manifest versions ([`fb87360`](https://github.com/saskenuba/SteamHelper-rs/commit/fb87360214c2f6d1319f467b82b27706ae157111))
    - Added accept/deny mobile confirmations on CLI. ([`fdcf407`](https://github.com/saskenuba/SteamHelper-rs/commit/fdcf4076fe266964f5e8c9aa5beb81ab38281a51))
    - Decoupled disk logic into fn `read_from_disk` on utils, + ([`0fc7ca6`](https://github.com/saskenuba/SteamHelper-rs/commit/0fc7ca6876a61d07945a4f6d5a0a937a44fe6af2))
    - Updated README with badges and minor fixes ([`14404f4`](https://github.com/saskenuba/SteamHelper-rs/commit/14404f4fd83c4c74893e3888693398d98bc3f199))
    - Readme changes ([`5be4545`](https://github.com/saskenuba/SteamHelper-rs/commit/5be4545d48846cf7e6ba166a545ce77fd451b26a))
    - Add convenience fn `has_trade_offer_id` to Confirmations ([`23f13a9`](https://github.com/saskenuba/SteamHelper-rs/commit/23f13a9e8927375f8a5dcd5be005e1c878132157))
    - (mobile, trading): fixes to manifest ([`43c3984`](https://github.com/saskenuba/SteamHelper-rs/commit/43c3984bf594bf6eb3d82c7c955e0b35d8db3d48))
    - Renamed from steam-auth to steam-mobile because of crates.io ([`749e6fc`](https://github.com/saskenuba/SteamHelper-rs/commit/749e6fc8c36af282ba18492e0b9f9f53ec7d00ed))
</details>

