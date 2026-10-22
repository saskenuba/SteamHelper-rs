//! Checks on Steam Community responses, before their body is read as a success.

use serde::Deserialize;
use steam_mobile::errors::InternalError as MobileInternalError;
use steam_mobile::StatusCode;
use steam_mobile::Url;
use steam_mobile::STEAM_COMMUNITY_HOST;

use crate::errors::error_of_strmessage;
use crate::errors::InternalError;
use crate::TradeError;

/// How much of an error page goes into the error, as in steam-mobile.
const ERROR_BODY_SNIPPET_CHARS: usize = 200;
/// How the path of Steam's login page starts, as steam-mobile checks it (`url_expired_check`).
const LOGIN_PATH: &str = "/login";

#[derive(Deserialize)]
struct StrError {
    #[serde(rename = "strError")]
    str_error: Option<String>,
}

/// Fails on Steam's errors, most specific first:
/// - HTTP 429: Steam's firewall; the body is its page, not an answer to the request;
/// - a `strError` in the body. Successful bodies never hold one, but the success types would parse it as a success
///   since all their fields are optional. Steam sends it with an error status too (node-steam-tradeoffer-manager reads
///   it on non-200 answers), and its `(N)` code is more useful than the status;
/// - any other status that is not 2xx.
pub(crate) fn check_response(status: StatusCode, url: &Url, body: &str) -> Result<(), TradeError> {
    rate_limit_check(status, url)?;

    if let Some(message) = str_error(body) {
        return Err(error_of_strmessage(message));
    }

    status_check(status, url, body)
}

/// Fails with steam-mobile's `SessionExpired` if Steam redirected the request to its login page: the session is gone,
/// and the page isn't an answer to the request. `url` is the answer's, after the redirects reqwest followed.
///
/// The login page is a Steam Community URL whose path starts with `/login`, as steam-mobile's `url_expired_check`
/// reads a `Location`. Seen live (2026-10-22): the privacy page's redirects ended on
/// `https://steamcommunity.com/login/home/?goto=…`. Checking the final URL is this crate's rule. The siblings check an
/// unfollowed redirect's `Location` for `/login` (node-steamcommunity `components/http.js:106-111`, aiosteampy
/// `transport/base.py:239`), and node-steamcommunity catches a lost session on a followed GET from the page itself
/// (`components/http.js:139-144`).
pub(crate) fn login_redirect_check(url: &Url) -> Result<(), TradeError> {
    if url.host_str() == Some(STEAM_COMMUNITY_HOST) && url.path().starts_with(LOGIN_PATH) {
        return Err(mobile_error(MobileInternalError::SessionExpired));
    }
    Ok(())
}

/// Fails on HTTP 429.
pub(crate) fn rate_limit_check(status: StatusCode, url: &Url) -> Result<(), TradeError> {
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(mobile_error(MobileInternalError::TooManyRequests {
            url: loggable_url(url),
        }));
    }
    Ok(())
}

/// Fails on a status that is not 2xx.
pub(crate) fn status_check(status: StatusCode, url: &Url, body: &str) -> Result<(), TradeError> {
    rate_limit_check(status, url)?;
    if status.is_success() {
        return Ok(());
    }
    Err(mobile_error(MobileInternalError::HttpStatus {
        status,
        url: loggable_url(url),
        body: body_snippet(body),
    }))
}

fn str_error(body: &str) -> Option<String> {
    serde_json::from_str::<StrError>(body).ok()?.str_error
}

fn mobile_error(error: MobileInternalError) -> TradeError {
    InternalError::MobileInternalError(error).into()
}

/// The URL without its query, which may hold a trade token.
fn loggable_url(url: &Url) -> String {
    let mut url = url.clone();
    url.set_query(None);
    url.to_string()
}

/// The start of an error page, whitespace collapsed.
fn body_snippet(body: &str) -> String {
    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    match collapsed.char_indices().nth(ERROR_BODY_SNIPPET_CHARS) {
        Some((end, _)) => format!("{}…", &collapsed[..end]),
        None => collapsed,
    }
}

#[cfg(test)]
mod tests {
    use steam_language::enums::EResult;

    use super::*;
    use crate::types::trade_offer_web::TradeOfferCancelResponse;
    use crate::types::trade_offer_web::TradeOfferCreateResponse;

    fn url() -> Url {
        Url::parse("https://steamcommunity.com/tradeoffer/new/?partner=1&token=secret").unwrap()
    }

    fn mobile(error: TradeError) -> MobileInternalError {
        match error {
            TradeError::InternalError(InternalError::MobileInternalError(error)) => error,
            other => panic!("not a steam-mobile error: {:?}", other),
        }
    }

    #[test]
    fn str_error_is_an_error_before_the_success_type() {
        let body = r#"{"strError":"There was an error accepting this trade offer. Please try again later. (26)"}"#;

        // Why the check comes first: the success types take it.
        assert!(serde_json::from_str::<TradeOfferCancelResponse>(body).is_ok());
        assert!(serde_json::from_str::<TradeOfferCreateResponse>(body).is_ok());

        for status in &[StatusCode::OK, StatusCode::INTERNAL_SERVER_ERROR] {
            match check_response(*status, &url(), body) {
                Err(TradeError::Steam {
                    eresult: EResult::Revoked,
                    message: Some(message),
                }) => assert!(message.ends_with("(26)"), "{}", message),
                other => panic!("{:?}", other),
            }
        }
    }

    #[test]
    fn str_error_without_known_code() {
        let body = r#"{"strError":"Você não pode negociar (aguarde)."}"#;
        match check_response(StatusCode::OK, &url(), body) {
            Err(TradeError::UnexpectedResponse(message)) => {
                assert_eq!(message, "Você não pode negociar (aguarde).")
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn success_bodies_pass() {
        for body in &[
            r#"{"tradeofferid":"4127436568"}"#,
            r#"{"tradeofferid":"4112828817","needs_mobile_confirmation":true,"needs_email_confirmation":false}"#,
            r#"{"tradeid":"3151905948734426645"}"#,
            r#"{"strError":null,"tradeofferid":"1"}"#,
            "",
            "<html></html>",
        ] {
            assert!(check_response(StatusCode::OK, &url(), body).is_ok(), "{}", body);
        }
    }

    #[test]
    fn a_redirect_to_the_login_page_is_session_expired() {
        // the final URL of the privacy page, live on 2026-10-22
        let login =
            "https://steamcommunity.com/login/home/?goto=%2Fprofiles%2F76561197961265729%2Ftradeoffers%2Fprivacy";
        match mobile(login_redirect_check(&Url::parse(login).unwrap()).unwrap_err()) {
            MobileInternalError::SessionExpired => {}
            other => panic!("{:?}", other),
        }

        for asked in &[
            "https://steamcommunity.com/profiles/76561197961265729/tradeoffers/privacy",
            // where `/profiles/…` redirects for an account whose custom URL starts with "login"
            "https://steamcommunity.com/id/loginbot/tradeoffers/privacy",
            "https://steamcommunity.com/inventory/76561197961265729/730/2?l=english&count=1000",
            "https://steamcommunity.com/tradeoffer/new/send",
            "https://steamcommunity.com/tradeoffer/9438483375/accept",
            "https://steamcommunity.com/trade/new/acknowledge",
            // only Steam Community's login page was seen
            "https://store.steampowered.com/login/",
        ] {
            assert!(login_redirect_check(&Url::parse(asked).unwrap()).is_ok(), "{}", asked);
        }
    }

    #[test]
    fn too_many_requests() {
        let error = check_response(StatusCode::TOO_MANY_REQUESTS, &url(), "<html>Access Denied</html>").unwrap_err();
        assert!(error.is_too_many_requests());
        match mobile(error) {
            MobileInternalError::TooManyRequests { url } => {
                assert_eq!(url, "https://steamcommunity.com/tradeoffer/new/")
            }
            other => panic!("{:?}", other),
        }

        // Even with a strError: the firewall answered, not the trade offer service.
        let error = check_response(StatusCode::TOO_MANY_REQUESTS, &url(), r#"{"strError":"x (26)"}"#).unwrap_err();
        assert!(error.is_too_many_requests());
    }

    #[test]
    fn other_error_status() {
        let page = format!("<html>\n  <body>{}</body>\n</html>", "é".repeat(300));
        let error = check_response(StatusCode::BAD_GATEWAY, &url(), &page).unwrap_err();
        assert!(!error.is_too_many_requests());
        match mobile(error) {
            MobileInternalError::HttpStatus { status, url, body } => {
                assert_eq!(status, StatusCode::BAD_GATEWAY);
                assert_eq!(url, "https://steamcommunity.com/tradeoffer/new/");
                assert!(body.starts_with("<html> <body>éé"));
                assert_eq!(body.chars().count(), ERROR_BODY_SNIPPET_CHARS + 1);
            }
            other => panic!("{:?}", other),
        }

        // A success body with an error status is still an error.
        assert!(check_response(StatusCode::INTERNAL_SERVER_ERROR, &url(), r#"{"tradeofferid":"1"}"#).is_err());
        assert!(check_response(StatusCode::FORBIDDEN, &url(), "").is_err());
    }
}
