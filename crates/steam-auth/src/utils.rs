use cookie::CookieJar;
use reqwest::{Response, StatusCode};

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

    Some(
        jar.iter()
            .filter(|c| c.domain() == Some(domain))
            .map(|c| format!("{}={}; ", c.name(), c.value()))
            .collect::<String>()
            .strip_suffix("; ")
            .unwrap()
            .to_string(),
    )
}

/// Retrieve cookie from jar,  filtered by domain and name, and them dumps into String.
pub fn dump_cookies_by_name(jar: &CookieJar, domain: &str, name: &str) -> Option<String> {
    jar.iter().peekable().peek()?;

    Some(
        jar.iter()
            .filter(|c| c.domain() == Some(domain))
            .filter(|c| c.name() == name)
            .map(|c| c.value())
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
