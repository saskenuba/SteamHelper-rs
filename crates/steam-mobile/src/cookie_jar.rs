//! The cookies [`crate::client::MobileClient`] sends, keyed by the exact host and the cookie name.
//!
//! Steam sets cookies of the same name (`sessionid`, `steamLoginSecure`) on each of its hosts, so a jar keyed by name
//! alone, as cookie-rs's `CookieJar` is, keeps only one of them. The fork of it this crate used before
//! (`cookie-hashed-domain`) hashed name and domain but compared by name only, so a same-named cookie on another host was
//! now and then replaced at random.
//!
//! The rules are the ones that jar applied when it lost nothing, and no others: a cookie is sent only to the host it
//! was stored under, compared as a plain string (no Domain or Path matching, no expiry, no `Secure`). The mobile app's
//! own cookies are defaults that win over a stored cookie of the same host and name; within each layer the latest
//! write wins.
//!
//! Order: a host's cookies come defaults first, then stored ones, each in the order of their first write; a cookie
//! that is written again keeps its place. Steam doesn't depend on the order (the old jar's was random per jar).

use cookie::Cookie;

#[derive(Default)]
pub struct CookieJar {
    /// The mobile app's cookies.
    defaults: Vec<Entry>,
    /// Cookies from responses' Set-Cookie headers, and from the session.
    stored: Vec<Entry>,
}

struct Entry {
    host: String,
    name: String,
    value: String,
}

impl CookieJar {
    /// Adds a cookie that a stored one of the same host and name doesn't replace.
    pub fn add_default(&mut self, host: &str, name: &str, value: &str) {
        upsert(&mut self.defaults, host, name, value);
    }

    /// Stores a cookie, replacing a stored one of the same host and name.
    ///
    /// `host` is matched exactly against the host of each request, so one with a leading dot is never sent.
    pub fn store(&mut self, host: &str, name: &str, value: &str) {
        upsert(&mut self.stored, host, name, value);
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
        find(&self.defaults, host, name)
            .or_else(|| find(&self.stored, host, name))
            .map(|entry| entry.value.as_str())
    }

    /// The cookies sent to `host`, as (name, value), in the order described in the module docs.
    pub fn cookies_on<'a>(&'a self, host: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        let defaults = self.defaults.iter().filter(move |entry| entry.host == host);
        let stored = self
            .stored
            .iter()
            .filter(move |entry| entry.host == host && find(&self.defaults, host, &entry.name).is_none());
        defaults
            .chain(stored)
            .map(|entry| (entry.name.as_str(), entry.value.as_str()))
    }

    /// Every cookie the jar sends, as (host, name, value).
    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str, &str)> {
        let stored = self
            .stored
            .iter()
            .filter(move |entry| find(&self.defaults, &entry.host, &entry.name).is_none());
        self.defaults
            .iter()
            .chain(stored)
            .map(|entry| (entry.host.as_str(), entry.name.as_str(), entry.value.as_str()))
    }
}

fn find<'a>(entries: &'a [Entry], host: &str, name: &str) -> Option<&'a Entry> {
    entries.iter().find(|entry| entry.host == host && entry.name == name)
}

fn upsert(entries: &mut Vec<Entry>, host: &str, name: &str, value: &str) {
    match entries
        .iter_mut()
        .find(|entry| entry.host == host && entry.name == name)
    {
        Some(entry) => value.clone_into(&mut entry.value),
        None => entries.push(Entry {
            host: host.to_owned(),
            name: name.to_owned(),
            value: value.to_owned(),
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

    #[test]
    fn a_cookie_is_stored_under_its_domain() {
        let mut jar = CookieJar::default();
        jar.store_cookie(&Cookie::build("sessionid", "abc").domain(COMMUNITY).path("/").finish());
        jar.store_cookie(&Cookie::new("orphan", "x"));

        assert_eq!(jar.get(COMMUNITY, "sessionid"), Some("abc"));
        assert_eq!(jar.iter().count(), 1);
    }
}
