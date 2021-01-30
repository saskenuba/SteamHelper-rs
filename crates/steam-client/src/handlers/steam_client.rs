use steam_language_gen::generated::enums::EMsg;

use crate::{handlers::HandlerKind, messages::packet::PacketMessage};

// handles
struct SteamClient {}

impl SteamClient {
    fn handle_logon_response() {}
    fn handle_logged_off() {}
    fn handle_login_key() {}
}

impl HandlerKind for SteamClient {
    fn handle_msg(packet_message: PacketMessage) {
        match packet_message.emsg() {
            EMsg::ClientLogOnResponse => Self::handle_logon_response(),
            EMsg::ClientLoggedOff => Self::handle_logged_off(),
            EMsg::ClientNewLoginKey => Self::handle_login_key(),
            // EMsg::ClientSessionToken => HandleSessionToken,
            // EMsg::ClientUpdateMachineAuth => HandleUpdateMachineAuth,
            // EMsg::ClientAccountInfo => HandleAccountInfo,
            // EMsg::ClientWalletInfoUpdate => HandleWalletInfo,
            // EMsg::ClientMarketingMessageUpdate2 => HandleMarketingMessageUpdate,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use steam_protobuf::steam::{
        steammessages_base::CMsgProtoBufHeader, steammessages_clientserver_login::CMsgClientLogonResponse,
    };

    use super::*;

    #[test]
    fn aa() {
        let wat = CMsgClientLogonResponse::new();
        let teste = CMsgProtoBufHeader::new();
    }
}
