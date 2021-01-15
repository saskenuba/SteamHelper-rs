//! Responsible for parsing HTML documents for various events.

use std::str::FromStr;

use scraper::{Html, Selector};

use crate::{
    errors::ApiKeyError,
    web_handler::confirmation::{Confirmation, ConfirmationDetails, EConfirmationType},
};

/// Get all confirmations by parsing the document.
/// Returns all confirmations found.
pub(crate) fn confirmation_retrieve(confirmation_html: Html) -> Option<Vec<Confirmation>> {
    let confirmation_nodes_selector = Selector::parse("div.mobileconf_list_entry").unwrap();

    // early return if no confirmations are found
    let mut entries = confirmation_html.select(&confirmation_nodes_selector).peekable();
    entries.peek()?;

    let confirmations = entries
        .map(|element| {
            let confirmation_type = element
                .value()
                .attr("data-type")
                .map(|s| EConfirmationType::from_str(s).unwrap())
                .unwrap_or(EConfirmationType::Unknown);

            let tradeoffer_id = if let EConfirmationType::Trade = confirmation_type {
                element.value().attr("data-creator").map(|s| i64::from_str(s).unwrap())
            } else {
                None
            };

            Confirmation {
                id: element.value().attr("data-confid").unwrap().to_string(),
                key: element.value().attr("data-key").unwrap().to_string(),
                kind: confirmation_type,
                details: tradeoffer_id.map(|id| ConfirmationDetails {
                    trade_offer_id: Some(id),
                }),
            }
        })
        .collect::<Vec<Confirmation>>();

    Some(confirmations)
}

/// Parse a single confirmation details into a [ConfirmationDetails] struct.
///
/// Does not need to have error since we already have a confirmation, and it has to have details.
pub(crate) fn confirmation_details_single(confirmation_details_html: Html) -> ConfirmationDetails {
    let trade_selector = Selector::parse("div.mobileconf_trade_area").unwrap();
    let market_selector = Selector::parse("div.mobileconf_listing_prices").unwrap();

    let tradearea_node = confirmation_details_html.select(&trade_selector).next();
    if tradearea_node.is_some() {
        let tradeoffer_node_selector = Selector::parse("div.tradeoffer").unwrap();
        let id = confirmation_details_html
            .select(&tradeoffer_node_selector)
            .next()
            .unwrap()
            .value()
            .attr("id")
            .unwrap();

        // id is formatted as 'tradeofferid_4000979435', and we are interest only at the digits.
        let tradeofferid_parsed = id.find('_').map(|a| &id[a + 1..]).unwrap();

        ConfirmationDetails {
            trade_offer_id: Some(i64::from_str(tradeofferid_parsed).unwrap()),
        }
    } else if confirmation_details_html
        .select(&market_selector)
        .peekable()
        .peek()
        .is_some()
    {
        ConfirmationDetails { trade_offer_id: None }
    } else {
        unimplemented!()
        // ConfirmationDetails {
        //     trade_offer_id: None,
        // Normally this should be reported, but under some specific
        // circumstances we might actually receive this one
        // }
    }
}

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

    fn sample_multi_confirmation() -> &'static str {
        include_str!("../assets/multi_confirmation.html")
    }

    fn sample_empty_confirmation() -> &'static str {
        include_str!("../assets/empty_confirmation.html")
    }

    #[test]
    fn test_resolve_api_key_status() {
        let api_doc = Html::parse_document(include_str!("../assets/api_ok.html"));
        let api = api_key_resolve_status(api_doc).unwrap();
        assert_eq!(api, "D805666DF5E380C5F8A89B8F8A0814B8");
    }

    #[test]
    fn test_multi_confirmation() {
        let api_doc = Html::parse_document(include_str!("../assets/multi_confirmation.html"));
        let confirmations = confirmation_retrieve(api_doc);
        assert!(confirmations.is_some())
    }

    #[test]
    fn test_empty_confirmation() {
        let api_doc = Html::parse_document(include_str!("../assets/empty_confirmation.html"));
        let confirmations = confirmation_retrieve(api_doc);
        assert!(confirmations.is_none())
    }
}
