use cookie::{Cookie, CookieJar};
use reqwest::{Response, StatusCode};
use std::fs::OpenOptions;
use std::io::Read;
use std::path::PathBuf;

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
            Some((&c[name_separator + name_len + 1..end_separator]).to_string())
        })
}

/// Retrieve all cookies from jar filtered by domain, and them dumps into String, ready
/// to be inserted as a header value
pub fn dump_cookies_by_domain(jar: &CookieJar, domain: &str) -> Option<String> {
    jar.iter().peekable().peek()?;

    jar.iter()
        .filter(|c| c.domain() == Some(domain))
        .map(|c| format!("{}={}; ", c.name(), c.value()))
        .collect::<String>()
        .strip_suffix("; ")
        .map(ToString::to_string)
}

/// Retrieve cookie from jar,  filtered by domain and name, and them dumps into String.
pub fn dump_cookies_by_name(jar: &CookieJar, domain: &str, name: &str) -> Option<String> {
    jar.iter().peekable().peek()?;

    Some(
        jar.iter()
            .filter(|c| c.domain() == Some(domain))
            .filter(|c| c.name() == name)
            .map(Cookie::value)
            .collect::<String>(),
    )
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
///
/// # Panic
/// Will panic if file is not found.
pub fn read_from_disk<T>(path: T) -> String
where
    T: Into<PathBuf>,
{
    let mut file = OpenOptions::new()
        .read(true)
        .open(path.into())
        .expect("Failed to read file from disk. Is it there?");
    let mut buffer = String::new();

    file.read_to_string(&mut buffer).unwrap();
    buffer
}
