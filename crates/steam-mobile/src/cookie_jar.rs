//! The cookies [`crate::client::MobileClient`] sends, keyed by the exact host and the cookie name.
//!
//! Steam sets cookies of the same name (`sessionid`, `steamLoginSecure`) on each of its hosts, so a jar keyed by name
//! alone, as cookie-rs's `CookieJar` is, keeps only one of them. The fork of it this crate used before
//! (`cookie-hashed-domain`) hashed name and domain but compared by name only, so a same-named cookie on another host was
//! now and then replaced at random.
//!
//! The rules are the ones that jar applied when it lost nothing, plus `Max-Age`: a cookie is sent only to the host it
//! was stored under, compared as a plain string (no Domain or Path matching, no `Secure`). The mobile app's own
//! cookies are defaults that win over a stored cookie of the same host and name; within each layer the latest write
//! wins.
//!
//! Expiry: a Set-Cookie's `Max-Age` is honoured (RFC 6265 5.2.2): the cookie is sent until that many seconds of
//! wall-clock time have passed; an expired cookie is left out of requests and dropped on the next write. Steam sends
//! it on redirects too: `market/eligibilitycheck` set `webTradeEligibility=…; Max-Age=30` on a redirect that carried
//! only `browserid` (2026-10-22); with the session's cookies it set it without expiry, so in normal use it stays for
//! the session (2026-10-22b). `Expires` is not read: in every Set-Cookie seen from Steam (2026-10-20 and 2026-10-22
//! captures) it comes with a `Max-Age`, which wins over it (RFC 6265 5.3 step 3). A cookie without `Max-Age`, and
//! every cookie this crate writes, never expires.
//!
//! Not `Secure`: a request to `http://` on a Steam host would carry the host's cookies, `steamLoginSecure` included.
//! No Steam redirect to `http://` has been seen.
//!
//! Order: a host's cookies come defaults first, then stored ones, each in the order of their first write; a cookie
//! that is written again keeps its place, unless it had expired. Steam doesn't depend on the order (the old jar's was
//! random per jar).
//!
//! The jar is also the HTTP client's cookie store ([`ClientJar`]), so a redirect sends the same cookies as the first
//! request, and the Set-Cookie of every response, redirects included, lands here.

use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;
use std::time::SystemTime;

use cookie::Cookie;
use parking_lot::RwLock;
use reqwest::header::HeaderValue;
use reqwest::Url;
use tracing::trace;

use crate::utils::dump_cookies_by_domain;

#[derive(Default)]
pub struct CookieJar {
    /// The mobile app's cookies.
    defaults: Vec<Entry>,
    /// Cookies from responses' Set-Cookie headers, and from the session.
    stored: Vec<Entry>,
    /// How far tests moved the jar's clock past the real one.
    #[cfg(test)]
    clock_ahead: Duration,
}

struct Entry {
    host: String,
    name: String,
    value: String,
    /// When the cookie stops being sent; `None` never.
    expires: Option<SystemTime>,
}

impl Entry {
    fn is_live(&self, now: SystemTime) -> bool {
        match self.expires {
            Some(expires) => now < expires,
            None => true,
        }
    }
}

impl CookieJar {
    /// Adds a cookie that a stored one of the same host and name doesn't replace.
    pub fn add_default(&mut self, host: &str, name: &str, value: &str) {
        upsert(&mut self.defaults, host, name, value, None);
    }

    /// Stores a cookie that never expires, replacing a stored one of the same host and name.
    ///
    /// `host` is matched exactly against the host of each request, so one with a leading dot is never sent.
    pub fn store(&mut self, host: &str, name: &str, value: &str) {
        self.store_until(host, name, value, None);
    }

    /// Stores `cookie`, from a Set-Cookie header of a response from `host`, under that host, whatever its Domain says.
    /// Its `Max-Age`, if any, sets when it expires (see the module docs).
    pub fn store_received(&mut self, host: &str, cookie: &Cookie<'_>) {
        // `Cookie::parse` reads a negative Max-Age as 0. One past the clock's range never expires.
        let expires = cookie
            .max_age()
            .and_then(|max_age| self.now().checked_add(max_age.unsigned_abs()));
        self.store_until(host, cookie.name(), cookie.value(), expires);
    }

    /// Drops every expired cookie, then stores a cookie sent until `expires`.
    fn store_until(&mut self, host: &str, name: &str, value: &str, expires: Option<SystemTime>) {
        let now = self.now();
        self.stored.retain(|entry| entry.is_live(now));
        upsert(&mut self.stored, host, name, value, expires);
    }

    fn now(&self) -> SystemTime {
        let now = SystemTime::now();
        #[cfg(test)]
        let now = now + self.clock_ahead;
        now
    }

    /// Moves the jar's clock `by` ahead, so tests see cookies expire without waiting.
    #[cfg(test)]
    pub fn advance_clock(&mut self, by: Duration) {
        self.clock_ahead += by;
    }

    /// Stores `cookie` under its Domain, as [`CookieJar::store`]; one without a Domain matches no request, so it is
    /// left out.
    pub fn store_cookie(&mut self, cookie: &Cookie<'_>) {
        if let Some(host) = cookie.domain() {
            self.store(host, cookie.name(), cookie.value());
        }
    }

    /// The value of the cookie `name` sent to `host`.
    pub fn get(&self, host: &str, name: &str) -> Option<&str> {
        let now = self.now();
        find(&self.defaults, host, name)
            .or_else(|| find(&self.stored, host, name).filter(|entry| entry.is_live(now)))
            .map(|entry| entry.value.as_str())
    }

    /// The cookies sent to `host`, as (name, value), in the order described in the module docs.
    /// Expired cookies are left out.
    pub fn cookies_on<'a>(&'a self, host: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        let now = self.now();
        let defaults = self.defaults.iter().filter(move |entry| entry.host == host);
        let stored = self.stored.iter().filter(move |entry| {
            entry.host == host && entry.is_live(now) && find(&self.defaults, host, &entry.name).is_none()
        });
        defaults
            .chain(stored)
            .map(|entry| (entry.name.as_str(), entry.value.as_str()))
    }

    /// Every cookie the jar sends, as (host, name, value).
    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str, &str)> {
        let now = self.now();
        let stored = self
            .stored
            .iter()
            .filter(move |entry| entry.is_live(now) && find(&self.defaults, &entry.host, &entry.name).is_none());
        self.defaults
            .iter()
            .chain(stored)
            .map(|entry| (entry.host.as_str(), entry.name.as_str(), entry.value.as_str()))
    }
}

/// The Cookie header the jar sends to `host`, or `None` if it has no cookie for that host.
pub fn cookie_header(jar: &CookieJar, host: &str) -> Option<HeaderValue> {
    // the names and values come from Set-Cookie header values, or are ours (ASCII)
    dump_cookies_by_domain(jar, host)
        .map(|header| HeaderValue::from_bytes(header.as_bytes()).expect("cookies are valid header bytes"))
}

/// A [`CookieJar`] as the HTTP client's cookie store (`ClientBuilder::cookie_provider`).
///
/// reqwest 0.11 asks it for the Cookie header of a request sent without one (the login's protobuf calls) and of every
/// redirected request, where its header replaces the one the request had (reqwest drops that one first when the host
/// changes). It hands it the Set-Cookie headers of every response, redirects included: they are stored by
/// [`CookieJar::store_received`], under the host that answered, until their `Max-Age` runs out.
pub struct ClientJar(pub Arc<RwLock<CookieJar>>);

impl reqwest::cookie::CookieStore for ClientJar {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        let host = url
            .host_str()
            .expect("reqwest only requests http(s) URLs, which have a host");
        let mut jar = self.0.write();
        // parsed as reqwest's `Response::cookies` does, which leaves out one that can't be
        for header in cookie_headers {
            let cookie = std::str::from_utf8(header.as_bytes())
                .ok()
                .and_then(|text| Cookie::parse(text).ok());
            if let Some(cookie) = cookie {
                trace!("New cookie from: {}, name: {}", host, cookie.name());
                jar.store_received(host, &cookie);
            }
        }
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        cookie_header(&self.0.read(), url.host_str()?)
    }
}

fn find<'a>(entries: &'a [Entry], host: &str, name: &str) -> Option<&'a Entry> {
    entries.iter().find(|entry| entry.host == host && entry.name == name)
}

fn upsert(entries: &mut Vec<Entry>, host: &str, name: &str, value: &str, expires: Option<SystemTime>) {
    match entries
        .iter_mut()
        .find(|entry| entry.host == host && entry.name == name)
    {
        Some(entry) => {
            value.clone_into(&mut entry.value);
            entry.expires = expires;
        }
        None => entries.push(Entry {
            host: host.to_owned(),
            name: name.to_owned(),
            value: value.to_owned(),
            expires,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMUNITY: &str = "steamcommunity.com";
    const STORE: &str = "store.steampowered.com";

    fn header(jar: &CookieJar, host: &str) -> String {
        jar.cookies_on(host)
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    #[test]
    fn defaults_beat_stored_cookies_whichever_comes_first() {
        let mut jar = CookieJar::default();
        jar.store(COMMUNITY, "mobileClient", "ios");
        jar.add_default(COMMUNITY, "mobileClient", "android");
        jar.store(COMMUNITY, "mobileClient", "web");

        assert_eq!(jar.get(COMMUNITY, "mobileClient"), Some("android"));
        assert_eq!(header(&jar, COMMUNITY), "mobileClient=android");
        // only on that host
        jar.store(STORE, "mobileClient", "web");
        assert_eq!(header(&jar, STORE), "mobileClient=web");
    }

    #[test]
    fn a_later_write_replaces_the_value_and_keeps_the_place() {
        let mut jar = CookieJar::default();
        jar.store(COMMUNITY, "sessionid", "a");
        jar.store(COMMUNITY, "steamLoginSecure", "token");
        jar.store(COMMUNITY, "sessionid", "b");

        assert_eq!(jar.get(COMMUNITY, "sessionid"), Some("b"));
        assert_eq!(header(&jar, COMMUNITY), "sessionid=b; steamLoginSecure=token");
    }

    #[test]
    fn the_same_name_on_two_hosts_stays_separate() {
        let mut jar = CookieJar::default();
        jar.store(COMMUNITY, "sessionid", "community");
        jar.store(STORE, "sessionid", "store");
        jar.store(COMMUNITY, "sessionid", "community2");

        assert_eq!(jar.get(COMMUNITY, "sessionid"), Some("community2"));
        assert_eq!(jar.get(STORE, "sessionid"), Some("store"));
        assert_eq!(jar.iter().count(), 2);
    }

    #[test]
    fn hosts_match_exactly() {
        let mut jar = CookieJar::default();
        jar.store(".store.steampowered.com", "steamparental", "pin");
        jar.store("steampowered.com", "browserid", "1");

        assert_eq!(header(&jar, STORE), "");
        assert_eq!(jar.get(STORE, "steamparental"), None);
        assert_eq!(jar.get(STORE, "browserid"), None);
    }

    #[test]
    fn the_header_lists_defaults_first_then_stored_cookies_in_order_of_first_write() {
        let mut jar = CookieJar::default();
        jar.store(COMMUNITY, "b", "1");
        jar.add_default(COMMUNITY, "z", "1");
        jar.store(STORE, "c", "1");
        jar.store(COMMUNITY, "a", "1");
        jar.add_default(COMMUNITY, "y", "1");
        jar.store(COMMUNITY, "b", "2");

        assert_eq!(header(&jar, COMMUNITY), "z=1; y=1; b=2; a=1");
        assert_eq!(header(&jar, STORE), "c=1");
    }

    fn received(jar: &mut CookieJar, host: &str, set_cookie: &str) {
        jar.store_received(host, &Cookie::parse(set_cookie).unwrap());
    }

    #[test]
    fn a_received_cookie_is_sent_until_its_max_age_runs_out() {
        let mut jar = CookieJar::default();
        jar.store(COMMUNITY, "sessionid", "s1");
        // as `market/eligibilitycheck` set it live on a redirect that carried only `browserid` (2026-10-22), Expires
        // next to Max-Age
        received(
            &mut jar,
            COMMUNITY,
            "webTradeEligibility=e1; expires=Thu, 22 Oct 2026 21:17:13 GMT; Max-Age=30; path=/; HttpOnly",
        );
        jar.store(COMMUNITY, "browserid", "b1");
        assert_eq!(
            header(&jar, COMMUNITY),
            "sessionid=s1; webTradeEligibility=e1; browserid=b1"
        );

        jar.advance_clock(Duration::from_secs(29));
        assert_eq!(jar.get(COMMUNITY, "webTradeEligibility"), Some("e1"));
        jar.advance_clock(Duration::from_secs(1));
        assert_eq!(header(&jar, COMMUNITY), "sessionid=s1; browserid=b1");
        assert_eq!(jar.get(COMMUNITY, "webTradeEligibility"), None);

        // set again without Max-Age (as with the session's cookies, 2026-10-22b): a new cookie, which never expires
        received(&mut jar, COMMUNITY, "webTradeEligibility=e2; path=/; HttpOnly");
        // steamLoginSecure's Max-Age, five years
        jar.advance_clock(Duration::from_secs(157_680_000));
        assert_eq!(
            header(&jar, COMMUNITY),
            "sessionid=s1; browserid=b1; webTradeEligibility=e2"
        );
    }

    #[test]
    fn a_cookie_set_again_without_max_age_no_longer_expires() {
        let mut jar = CookieJar::default();
        // both shapes Steam sent for this cookie (2026-10-22 and 2026-10-22b)
        received(
            &mut jar,
            COMMUNITY,
            "webTradeEligibility=e1; Max-Age=30; path=/; HttpOnly",
        );
        jar.advance_clock(Duration::from_secs(10));
        received(&mut jar, COMMUNITY, "webTradeEligibility=e2; path=/; HttpOnly");

        jar.advance_clock(Duration::from_secs(30));
        assert_eq!(jar.get(COMMUNITY, "webTradeEligibility"), Some("e2"));
        assert_eq!(header(&jar, COMMUNITY), "webTradeEligibility=e2");
    }

    #[test]
    fn a_max_age_past_the_clocks_range_never_expires() {
        let mut jar = CookieJar::default();
        // `Cookie::parse` caps it at i64::MAX seconds
        received(&mut jar, COMMUNITY, "browserid=b1; Max-Age=99999999999999999999");
        jar.advance_clock(Duration::from_secs(157_680_000));
        assert_eq!(jar.get(COMMUNITY, "browserid"), Some("b1"));
    }

    #[test]
    fn a_cookie_is_stored_under_its_domain() {
        let mut jar = CookieJar::default();
        jar.store_cookie(&Cookie::build("sessionid", "abc").domain(COMMUNITY).path("/").finish());
        jar.store_cookie(&Cookie::new("orphan", "x"));

        assert_eq!(jar.get(COMMUNITY, "sessionid"), Some("abc"));
        assert_eq!(jar.iter().count(), 1);
    }
}
