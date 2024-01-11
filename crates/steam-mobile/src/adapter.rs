use cookie::Cookie;
use derive_more::Deref;
use derive_more::DerefMut;

#[derive(Deref, DerefMut, Clone, Debug)]
pub struct SteamCookie(Cookie<'static>);

impl From<reqwest::cookie::Cookie<'_>> for SteamCookie {
    fn from(value: reqwest::cookie::Cookie) -> Self {
        let mut cookie = Cookie::build(value.name().to_owned(), value.value().to_owned()).path("/");

        if let Some(domain) = value.domain() {
            cookie = cookie.domain(domain.to_owned());
        }

        Self(cookie.finish())
    }
}
