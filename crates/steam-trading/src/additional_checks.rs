use scraper::Html;
use scraper::Selector;
use steam_mobile::Method;
use steamid_parser::SteamID;

use crate::errors::InternalError;
use crate::OfferError;
use crate::SteamCompleteAuthenticator;
use crate::TradeError;
use crate::TryFutureExt;
use crate::TRADEOFFER_BASE;

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

pub(crate) async fn check_steam_guard_error(
    authenticator: &SteamCompleteAuthenticator,
    steamid: SteamID,
    token: &str,
) -> Result<(), TradeError> {
    let endpoint = format!(
        "{}new/?partner={}&token={}",
        TRADEOFFER_BASE,
        steamid.to_steam3(),
        token
    );

    let response = authenticator
        .request_custom_endpoint(endpoint, Method::GET, None, None::<&u8>)
        .err_into::<InternalError>()
        .await?;
    let text = response.text().err_into::<InternalError>().await?;

    if is_steam_guard_error(&text) {
        Err(OfferError::SteamGuardRecentlyEnabled.into())
    } else {
        Ok(())
    }
}
