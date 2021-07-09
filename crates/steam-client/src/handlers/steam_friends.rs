use steam_language_gen::generated::enums::{EMsg, EPersonaState};
use steam_protobuf::steam::steammessages_clientserver_friends::CMsgClientChangeStatus;

use crate::messages::message::ClientMessage;

enum SteamFriendsEvents {}

struct SteamFriends;

impl SteamFriends {
    pub fn set_persona() {
        // pega o channel, prepara e envia
    }
}

fn set_persona_state(state: EPersonaState) -> ClientMessage<CMsgClientChangeStatus> {
    let mut persona_message: ClientMessage<CMsgClientChangeStatus> = ClientMessage::new_proto(EMsg::ClientChangeStatus);
    persona_message.body.set_persona_state(state as u32);

    persona_message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aaaa() {
        let teste = set_persona_state(EPersonaState::Online);
        dbg!(&teste);
    }
}
