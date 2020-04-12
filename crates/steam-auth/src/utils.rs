use cookie::CookieJar;

/// Retrieve all cookies filtered by domain, and them dumps into String, ready
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
