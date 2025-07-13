//! Responsible for parsing HTML documents for various events.

use scraper::Html;
use scraper::Selector;

use crate::errors::ApiKeyError;

/// Checks API Key state by parsing the document.
/// If key is found, returns it, otherwise, it just errors accordingly.
pub(crate) fn api_key_resolve_status(api_key_html: Html) -> Result<String, ApiKeyError> {
    let api_page_title_selector = Selector::parse("#mainContents > h2").unwrap();
    let api_page_key_selector = Selector::parse("#bodyContents_ex > p:nth-child(2)").unwrap();

    let title = match api_key_html.select(&api_page_title_selector).next() {
        None => return Err(ApiKeyError::GeneralError("title is blank".to_string())),
        Some(title) => title.text().collect::<String>(),
    };

    if title.contains("Validated email address required") || title.contains("Access Denied") {
        return Err(ApiKeyError::AccessDenied);
    }

    let api_key_text = match api_key_html.select(&api_page_key_selector).next() {
        None => return Err(ApiKeyError::GeneralError("key node is blank".to_string())),
        Some(api_key_text) => api_key_text.text().collect::<String>(),
    };

    if api_key_text.contains("Registering for a Steam Web API Key") {
        return Err(ApiKeyError::NotRegistered);
    }

    let api_key = api_key_text.split("Key: ").nth(1).unwrap();

    if api_key.len() != 32 {
        return Err(ApiKeyError::GeneralError(format!(
            "Size should be 32. Found: {}",
            api_key
        )));
    }

    Ok(api_key.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn sample_multi_confirmation() -> &'static str {
        include_str!("../assets/multi_confirmation.html")
    }

    const fn sample_empty_confirmation() -> &'static str {
        include_str!("../assets/empty_confirmation.html")
    }

    #[test]
    fn test_resolve_api_key_status() {
        let api_doc = Html::parse_document(include_str!("../assets/api_ok.html"));
        let api = api_key_resolve_status(api_doc).unwrap();
        assert_eq!(api, "D805666DF5E380C5F8A89B8F8A0814B8");
    }

    // #[test]
    // fn test_multi_confirmation() {
    //     let api_doc = Html::parse_document(include_str!("../assets/multi_confirmation.html"));
    //     let confirmations = confirmation_retrieve(api_doc);
    //     assert!(confirmations.is_some());
    // }

    // #[test]
    // fn test_empty_confirmation() {
    //     let api_doc = Html::parse_document(include_str!("../assets/empty_confirmation.html"));
    //     let confirmations = confirmation_retrieve(api_doc);
    //     assert!(confirmations.is_none());
    // }
}
