use reqwest::header::{HeaderMap, HeaderValue};
use thiserror::Error;

use steam_language::enums::EResult;

#[derive(Debug, Error)]
pub enum SteamAPIError {
    #[error("`An error EResult was found: {1}`")]
    EResult(EResult, String),

    /// The `x-eresult` header holds a value that is not a known [EResult].
    #[error("An unknown X-eresult was found: `{0}`")]
    UnknownEResult(String),

    #[error("An X-error_message was found: `{0}`")]
    ErrorMessage(String),

    #[error("`{0}`")]
    SteamHttpError(String),

    /// Its URL has no query string: the query carries the API key or the access token.
    #[error(transparent)]
    HttpError(reqwest::Error),

    /// The response passed the error checks but its body doesn't match the response type.
    /// `body` is the raw response, to tell a Steam change from a Steam hiccup.
    #[error("Failed to decode the response: {source}")]
    DecodeError { source: serde_json::Error, body: String },
}

/// `reqwest::Error` shows its URL in `Display` and `Debug`, and a request's query holds `key=` or `access_token=`, so
/// the query is removed. The path still tells which endpoint failed.
impl From<reqwest::Error> for SteamAPIError {
    fn from(mut error: reqwest::Error) -> Self {
        if let Some(url) = error.url_mut() {
            url.set_query(None);
        }
        SteamAPIError::HttpError(error)
    }
}

/// Header values are not guaranteed to be ASCII, so `HeaderValue::to_str` can't be unwrapped.
fn header_to_string(value: &HeaderValue) -> String {
    String::from_utf8_lossy(value.as_bytes()).into_owned()
}

/// Steam has been seen answering `x-eresult: 2` (Fail) to calls that worked. Mirrors
/// node-steam-tradeoffer-manager (`lib/webapi.js:56-66`): the 2 is ignored when the body has more than one top level
/// key, or a non-empty `response` object.
fn is_fake_fail(body: &str) -> bool {
    let body: serde_json::Value = match serde_json::from_str(body) {
        Ok(body) => body,
        Err(_) => return false,
    };
    let body = match body.as_object() {
        Some(body) => body,
        None => return false,
    };

    body.len() > 1
        || body
            .get("response")
            .and_then(|response| response.as_object())
            .is_some_and(|response| !response.is_empty())
}

/// Main error function that checks the response status and two response headers: `X-eresult` and `X-error_message`.
///
/// The following priority is applied for errors:
/// - If the header X-error_message is found, the error displayed X-error_message;
/// - If header X-eresult is returned and is not an OK (1), then the EResult (for example, NoMatch) is shown, and can be
///   matched. A Fail (2) with a non-empty body is taken as OK (see [is_fake_fail]);
/// - If the HTTP status code is not 200, this is the HTTP status message (for example, Not Found or Unauthorized),
///   even if X-eresult is OK.
pub(crate) fn headers_error_check(
    status_code: reqwest::StatusCode,
    response_headers: &HeaderMap,
    body: &str,
) -> Result<(), SteamAPIError> {
    let eresult_header = "x-eresult";
    let error_message = "X-error_message";

    let status_code = status_code.as_u16();

    if let Some(err) = response_headers.get(error_message) {
        return Err(SteamAPIError::ErrorMessage(header_to_string(err)));
    };

    if let Some(header_value) = response_headers.get(eresult_header) {
        let header_value = header_to_string(header_value);
        let eresult = match serde_json::from_str::<EResult>(header_value.trim()) {
            Ok(eresult) => eresult,
            Err(_) => return Err(SteamAPIError::UnknownEResult(header_value)),
        };

        let fake_fail = eresult == EResult::Fail && is_fake_fail(body);
        if eresult != EResult::OK && !fake_fail {
            let error_description = format!("Value: {}", header_value);
            return Err(SteamAPIError::EResult(eresult, error_description));
        }
    }

    if status_code != 200 {
        return Err(SteamAPIError::SteamHttpError(status_code.to_string()));
    }

    Ok(())
}

/// Deserializes a body that passed [headers_error_check].
pub(crate) fn decode_response<T: serde::de::DeserializeOwned>(body: String) -> Result<T, SteamAPIError> {
    serde_json::from_str(&body).map_err(|source| SteamAPIError::DecodeError { source, body })
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::*;

    fn headers(pairs: &[(&'static str, &[u8])]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(*name, HeaderValue::from_bytes(value).unwrap());
        }
        headers
    }

    #[test]
    fn ok_without_eresult() {
        assert!(headers_error_check(StatusCode::OK, &HeaderMap::new(), "{}").is_ok());
    }

    #[test]
    fn ok_with_eresult_ok() {
        assert!(headers_error_check(StatusCode::OK, &headers(&[("x-eresult", b"1")]), "{}").is_ok());
    }

    #[test]
    fn eresult_error() {
        let result = headers_error_check(StatusCode::OK, &headers(&[("x-eresult", b"15")]), "{}");
        assert!(matches!(result, Err(SteamAPIError::EResult(EResult::AccessDenied, _))));
    }

    #[test]
    fn eresult_takes_priority_over_status() {
        let result = headers_error_check(StatusCode::FORBIDDEN, &headers(&[("x-eresult", b"15")]), "");
        assert!(matches!(result, Err(SteamAPIError::EResult(EResult::AccessDenied, _))));
    }

    #[test]
    fn status_error_without_eresult() {
        let result = headers_error_check(StatusCode::TOO_MANY_REQUESTS, &HeaderMap::new(), "");
        assert!(matches!(result, Err(SteamAPIError::SteamHttpError(s)) if s == "429"));
    }

    #[test]
    fn status_error_is_not_hidden_by_eresult_ok() {
        let result = headers_error_check(
            StatusCode::INTERNAL_SERVER_ERROR,
            &headers(&[("x-eresult", b"1")]),
            "{}",
        );
        assert!(matches!(result, Err(SteamAPIError::SteamHttpError(s)) if s == "500"));
    }

    #[test]
    fn error_message_first() {
        let result = headers_error_check(
            StatusCode::OK,
            &headers(&[("x-eresult", b"1"), ("x-error_message", b"bad tradeofferid")]),
            "{}",
        );
        assert!(matches!(result, Err(SteamAPIError::ErrorMessage(s)) if s == "bad tradeofferid"));
    }

    #[test]
    fn non_ascii_headers_do_not_panic() {
        let result = headers_error_check(
            StatusCode::OK,
            &headers(&[("x-error_message", b"caf\xc3\xa9 \xff")]),
            "{}",
        );
        assert!(matches!(result, Err(SteamAPIError::ErrorMessage(s)) if s == "café \u{fffd}"));

        let result = headers_error_check(StatusCode::OK, &headers(&[("x-eresult", b"\xff")]), "{}");
        assert!(matches!(result, Err(SteamAPIError::UnknownEResult(_))));
    }

    #[test]
    fn unknown_eresult_does_not_panic() {
        let result = headers_error_check(StatusCode::OK, &headers(&[("x-eresult", b"99999")]), "{}");
        assert!(matches!(result, Err(SteamAPIError::UnknownEResult(s)) if s == "99999"));
    }

    #[test]
    fn fail_with_content_is_ok() {
        let fail = headers(&[("x-eresult", b"2")]);
        let response_with_content = r#"{"response":{"trade_offers_sent":[]}}"#;
        let two_top_level_keys = r#"{"response":{},"other":1}"#;

        assert!(headers_error_check(StatusCode::OK, &fail, response_with_content).is_ok());
        assert!(headers_error_check(StatusCode::OK, &fail, two_top_level_keys).is_ok());
    }

    #[test]
    fn fail_without_content_is_an_error() {
        let fail = headers(&[("x-eresult", b"2")]);

        for body in &[
            "",
            "{}",
            r#"{"response":{}}"#,
            r#"{"response":[1]}"#,
            "[1, 2]",
            "not json",
        ] {
            let result = headers_error_check(StatusCode::OK, &fail, body);
            assert!(
                matches!(result, Err(SteamAPIError::EResult(EResult::Fail, _))),
                "body: {}",
                body
            );
        }
    }

    #[test]
    fn only_fail_is_forgiven() {
        let busy = headers(&[("x-eresult", b"10")]);
        let result = headers_error_check(StatusCode::OK, &busy, r#"{"response":{"a":1}}"#);
        assert!(matches!(result, Err(SteamAPIError::EResult(EResult::Busy, _))));
    }

    #[test]
    fn decode_error_keeps_the_body() {
        let result = decode_response::<crate::response_types::GetCMListResponseBase>("<html>".to_string());
        assert!(matches!(result, Err(SteamAPIError::DecodeError { body, .. }) if body == "<html>"));
    }
}
