use reqwest::header::{HeaderMap, HeaderValue};
use thiserror::Error;

use steam_language_gen::generated::enums::EResult;

#[derive(Debug, Error)]
pub enum SteamAPIError {
    #[error("`An error EResult was found: {1}`")]
    EResult(EResult, String),

    #[error("An X-error_message was found: `{0}`")]
    ErrorMessage(String),

    #[error("`{0}`")]
    SteamHttpError(String),

    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}

/// Main error function that checks for two response headers: `X-eresult` and `X-error_message`.
///
/// The following priority is applied for errors:
/// - If the header X-error_message is found, the error displayed X-error_message;
/// - If header X-eresult is returned and is not an OK (1), then the EResult (for example, NoMatch) is shown, and can be matched;
/// - If the HTTP status code is not 200, this is the HTTP status message (for example, Not Found or Unauthorized);
pub(crate) fn headers_error_check(
    status_code: reqwest::StatusCode,
    response_headers: &HeaderMap,
) -> Result<(), SteamAPIError> {
    let eresult_header = "x-eresult";
    let error_message = "X-error_message";

    let status_code = status_code.as_u16();

    if let Some(err) = response_headers.get(error_message) {
        return Err(SteamAPIError::ErrorMessage(err.to_str().unwrap().to_string()));
    };

    let eresult_header = response_headers.get(eresult_header);
    if eresult_header.is_none() {
        return if status_code == 200 {
            Ok(())
        } else {
            Err(SteamAPIError::SteamHttpError(status_code.to_string()))
        };
    }

    let header_value = eresult_header.unwrap().to_str().unwrap();

    // Safe to unwrap because eresult are always integers
    let value_into_eresult = serde_json::from_str::<EResult>(header_value).unwrap();

    if value_into_eresult != EResult::OK {
        let error_description = format!("Value: {}", header_value);
        return Err(SteamAPIError::EResult(value_into_eresult, error_description));
    }

    Ok(())
}
