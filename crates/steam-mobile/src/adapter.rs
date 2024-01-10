use cookie::Cookie;
use derive_more::Deref;

#[derive(Deref, Clone, Debug)]
pub(crate) struct SteamCookie(Cookie<'static>);

impl From<reqwest::cookie::Cookie<'_>> for SteamCookie {
    fn from(value: reqwest::cookie::Cookie) -> Self {
        let mut cookie = Cookie::build(value.name().to_owned(), value.value().to_owned()).path("/");

        if let Some(domain) = value.domain() {
            cookie = cookie.domain(domain.to_owned());
        }

        Self { 0: cookie.finish() }
    }
}
