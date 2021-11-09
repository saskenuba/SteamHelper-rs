use crate::{OfferError, TradeError, TryFutureExt, TRADEOFFER_BASE};
use scraper::{Html, Selector};
use steam_auth::client::SteamAuthenticator;
use steam_auth::Method;
use steamid_parser::SteamID;

fn is_steam_guard_error(document: &str) -> bool {
    let doc = Html::parse_document(document);

    // safe to unwrap
    let error_msg_block = Selector::parse("div#error_msg").unwrap();
    let mut error_message = match doc.select(&error_msg_block).next() {
        None => return false,
        Some(element) => element.text(),
    };

    if let Some(error_text) = error_message.next() {
        if error_text.contains("is not available to trade") {
            return true;
        }
    }

    false
}

pub async fn check_steam_guard_error(
    authenticator: &SteamAuthenticator,
    steamid: SteamID,
    token: &str,
) -> Result<(), TradeError> {
    let endpoint = format!(
        "{}new/?partner={}&token={}",
        TRADEOFFER_BASE,
        steamid.to_steam3(),
        token
    );

    authenticator.

    let response = authenticator
        .request_custom_endpoint(endpoint, Method::GET, None, None::<&u8>)
        .and_then(|x| x.text())
        .await?;

    if is_steam_guard_error(&response) {
        Err(OfferError::SteamGuardRecentlyEnabled.into())
    } else {
        Ok(())
    }
}
