use std::fs::OpenOptions;
use std::io::Read;
use std::path::PathBuf;

use reqwest::{Response, StatusCode};

use crate::cookie_jar::CookieJar;

const CAPTCHA_URL: &str = "https://steamcommunity.com/login/rendercaptcha/?gid=";

/// Formats the captcha GID into the complete URL.
/// E.g: https://steamcommunity.com/login/rendercaptcha/?gid=3851100575032057891
pub fn format_captcha_url(captcha_guid: &str) -> String {
    CAPTCHA_URL.to_owned() + captcha_guid
}

/// Generates a standard "Android Device ID" that is based on user's Steam ID.
pub fn generate_canonical_device_id(steamid: &str) -> String {
    steam_totp::get_device_id(steamid)
}

/// Retrieve cookie from header response filtered by name.
pub fn dump_cookie_from_header(response: &Response, name: &str) -> Option<String> {
    let name_len = name.len();

    response
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .map(|header_value| header_value.to_str().unwrap())
        .and_then(|c| {
            let name_separator = c.find(name)?;
            let end_separator = c.find(';')?;

            // + 1 here because of '=' sign
            Some(c[name_separator + name_len + 1..end_separator].to_string())
        })
}

/// The cookies the jar sends to the host `domain`, ready to be inserted as a Cookie header value, or `None` if it has
/// none for that host.
pub fn dump_cookies_by_domain(jar: &CookieJar, domain: &str) -> Option<String> {
    let header = jar
        .cookies_on(domain)
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ");
    Some(header).filter(|header| !header.is_empty())
}

/// The value of the cookie `name` the jar sends to the host `domain`.
///
/// `None` if the jar has no such cookie.
pub fn dump_cookies_by_domain_and_name(jar: &CookieJar, domain: &str, name: &str) -> Option<String> {
    jar.get(domain, name).map(ToString::to_string)
}

/// Returns the redirect url from the Location header from a response, or None if
/// Location header is not found.
pub fn retrieve_header_location(response: &Response) -> Option<&str> {
    let location_url = match response.status() {
        StatusCode::FOUND | StatusCode::PERMANENT_REDIRECT | StatusCode::TEMPORARY_REDIRECT => {
            response.headers().get(reqwest::header::LOCATION)
        }
        _ => return None,
    };
    Some(location_url.unwrap().to_str().unwrap())
}

/// Convenience function that imports the file from disk
pub fn read_from_disk<T>(path: T) -> std::io::Result<String>
where
    T: Into<PathBuf>,
{
    let mut file = OpenOptions::new().read(true).open(path.into())?;
    let mut buffer = String::new();

    file.read_to_string(&mut buffer)?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookies_by_domain_and_name() {
        let mut jar = CookieJar::default();
        assert_eq!(
            dump_cookies_by_domain_and_name(&jar, "steamcommunity.com", "sessionid"),
            None
        );

        jar.add_default("steamcommunity.com", "mobileClient", "android");
        // a jar with other cookies, but not this one: bug 12 sent this as an empty sessionid
        assert_eq!(
            dump_cookies_by_domain_and_name(&jar, "steamcommunity.com", "sessionid"),
            None
        );

        jar.store("store.steampowered.com", "sessionid", "abc");
        assert_eq!(
            dump_cookies_by_domain_and_name(&jar, "steamcommunity.com", "sessionid"),
            None
        );

        jar.store("steamcommunity.com", "sessionid", "def");
        assert_eq!(
            dump_cookies_by_domain_and_name(&jar, "steamcommunity.com", "sessionid").as_deref(),
            Some("def")
        );
    }

    #[test]
    fn cookies_by_domain() {
        let mut jar = CookieJar::default();
        jar.add_default("steamcommunity.com", "mobileClient", "android");
        jar.store("steamcommunity.com", "sessionid", "def");
        jar.store("store.steampowered.com", "sessionid", "abc");

        assert_eq!(
            dump_cookies_by_domain(&jar, "steamcommunity.com").as_deref(),
            Some("mobileClient=android; sessionid=def")
        );
        // `request` sends this as an empty Cookie header
        assert_eq!(dump_cookies_by_domain(&jar, "help.steampowered.com"), None);
    }
}
