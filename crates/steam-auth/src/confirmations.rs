use crate::COMMUNITY_BASE;
use scraper::Selector;
use steam_totp::generate_confirmation_key;

struct Confirmation {}

// 1. generate confirmation hash
// 2. get confirmations

enum ConfirmationType {
    Unknown,
    Generic,
    Trade,
    Market,

    // We're missing information about definition of number 4 type
    PhoneNumberChange = 5,
    AccountRecovery = 6,
}

fn get_web_confirmation_document() {}

/// Return html document that we can parse
fn get_confirmations(device_id: &str, confirmation_hash: &str, time: u32) {
    let my_steamid = "123";
    let confirmation_hash = ""; // url_encoded
    let device_id = ""; // url_encoded
    let time = ""; //?

    let confirmation_nodes_selector = Selector::parse("div.mobileconf_list_entry").unwrap();
    let id_text_selector = Selector::parse("data-confid").unwrap();
    let key_text_selector = Selector::parse("data-key").unwrap();
    let type_text_selector = Selector::parse("data-type").unwrap();

    let teste = format!(
        "{}/mobileconf/conf?a={}&k={}&l=english&m=android&p={}&t={}&tag=conf",
        COMMUNITY_BASE, my_steamid, confirmation_hash, device_id, time
    );
}

enum InventoryPrivacy {
    Unknown,
    Private,
    FriendsOnly,
    Public,
}

enum ApikeyState {
    Error,
    Registered,
    NotRegistered,
    AccessDenied,
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
}
