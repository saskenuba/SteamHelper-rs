use std::marker::PhantomData;

use futures::task::AtomicWaker;
use steam_language_gen::generated::enums::{EMsg, EOSType};
use steam_protobuf::steam::steammessages_base::CMsgIPAddress;
use steam_protobuf::steam::steammessages_clientserver_login::{
    CMsgClientLogon, CMsgClientLogonResponse, CMsgClientRequestWebAPIAuthenticateUserNonceResponse,
};

use crate::handlers::{DispatcherMap, HandlerKind};
use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;
use crate::messages::{ProtoMsgBox, ProtoRecover};
use steam_language_gen::SerializableBytes;
use steam_protobuf::Message;

/// Events are messages received from the network
enum SteamUserEvents {
    /// When account information is received from the network
    AccountInfo,
}

// handles
#[derive(Copy, Clone, Debug)]
pub struct SteamUser;

impl SteamUser {
    fn with_context<T>(cx: &DispatcherMap) -> SteamUserMessages {
        SteamUserMessages { dispatcher: cx }
    }
}

struct SteamUserMessages<'a> {
    dispatcher: &'a DispatcherMap,
}

impl<'a> SteamUserMessages<'a> {
    pub async fn log_on(&self) -> CMsgClientLogonResponse {
        let message = do_logon(LogOnDetails::default()).to_bytes();
        self.dispatcher.sender.send(message);

        // let response: ClientMessage<_> = AsyncResponseInner {
        //     dispatcher: self.dispatcher,
        //     source_job_id: 0,
        //     waker: AtomicWaker::new(),
        //     message: Default::default(),
        // }
        // .await;

        CMsgClientLogonResponse::new()
    }
}

struct AsyncResponseInner<'a, T> {
    dispatcher: &'a DispatcherMap,
    source_job_id: u64,
    waker: AtomicWaker,
    message: PhantomData<T>,
}

// impl<'a, T> Future for AsyncResponseInner<'a, T> {
//     type Output = ClientMessage<T>;
//
//     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         let source_job_id = self.source_job_id;
//         self.waker.register(cx.waker());
//
//         match self.dispatcher.tracked_protobuf_messages.get(&source_job_id) {
//             None => {}
//             Some(a) => {}
//         };
//
//         Poll::Ready(())
//     }
// }

impl<'a, T> AsyncResponseInner<'a, T> {
    pub async fn request_webapi_nonce(&self) -> CMsgClientRequestWebAPIAuthenticateUserNonceResponse {
        let wat = CMsgClientRequestWebAPIAuthenticateUserNonceResponse::new().boxed_any();

        let recovered = wat.recover::<CMsgClientRequestWebAPIAuthenticateUserNonceResponse>();

        CMsgClientRequestWebAPIAuthenticateUserNonceResponse::new()
    }
}

impl HandlerKind for SteamUser {
    fn handle_msg(packet_message: PacketMessage) {
        match packet_message.emsg() {
            EMsg::ClientLogOnResponse => (),
            EMsg::ClientLoggedOff => (),
            EMsg::ClientNewLoginKey => (),
            // EMsg::ClientSessionToken => HandleSessionToken,
            // EMsg::ClientUpdateMachineAuth => HandleUpdateMachineAuth,
            // EMsg::ClientAccountInfo => HandleAccountInfo,
            // EMsg::ClientWalletInfoUpdate => HandleWalletInfo,
            // EMsg::ClientMarketingMessageUpdate2 => HandleMarketingMessageUpdate,
            _ => {}
        }
    }
}

/// Details required to log into Steam3 as a user.
struct LogOnDetails {
    /// Steam username.
    username: String,
    /// Steam password.
    password: String,
    /// CellID is the region you are going to fetch Steam servers.
    cell_id: Option<u32>,
    /// Gets or sets the LoginID. This number is used for identifying logon session.
    /// The purpose of this field is to allow multiple sessions to the same steam account from the same machine.
    /// This is because Steam Network doesn't allow more than one session with the same LoginID to access given account
    /// at the same time from the same public IP. If you want to establish more than one active session to given
    /// account, you must make sure that every session (to that account) from the same public IP has a unique LoginID.
    /// By default LoginID is automatically generated based on machine's primary bind address, which is the same for
    /// all sessions. Null value will cause this property to be automatically generated based on default behaviour.
    /// If in doubt, set this property to null.
    login_id: u32,
    account_instance: u32,
    account_id: u32,

    login_key: String,
    should_remember_password: bool,

    /// Steam Guard code sent to user's email.
    auth_code: String,
    /// 2fa code used for login. Received from authenticator apps.
    two_auth_code: String,
    sentry_file_hash: Option<Vec<u8>>,

    /// The client operating system type.
    os_type: EOSType,
    /// Clien's language.
    client_language: String,
}

impl Default for LogOnDetails {
    fn default() -> Self {
        Self {
            account_id: 0,
            account_instance: 1,
            client_language: "english".to_string(),
            // FIXME: is necessary to have this?
            os_type: EOSType::LinuxUnknown,
            ..Default::default()
        }
    }
}

fn do_logon(logon_details: LogOnDetails) -> ClientMessage<CMsgClientLogon> {
    let mut logon_message: ClientMessage<CMsgClientLogon> = ClientMessage::new_proto(EMsg::ClientLogon);

    // let steamid = SteamID::new()

    let mut ip_addr_msg = CMsgIPAddress::new();

    // TODO: steam also sets ipv6
    let my_ip = 0;
    let ip_obfuscation_mask: u32 = 0xBAADF00D;
    let obfuscated_ip = my_ip ^ ip_obfuscation_mask;

    ip_addr_msg.set_v4(obfuscated_ip);
    logon_message.body.set_obfuscated_private_ip(ip_addr_msg);

    //
    logon_message.body.set_account_name(logon_details.username);
    logon_message.body.set_password(logon_details.password);
    logon_message
        .body
        .set_should_remember_password(logon_details.should_remember_password);
    logon_message.body.set_client_language(logon_details.client_language);
    logon_message.body.set_cell_id(logon_details.cell_id.unwrap_or(0));
    logon_message.body.set_steam2_ticket_request(false);
    logon_message.body.set_protocol_version(65580);
    logon_message.body.set_client_package_version(1771);
    logon_message.body.set_supports_rate_limit_response(true);

    // machine_id not needed apparently
    // logon_message.body.set_machine_id();

    // Steam Guard related:
    logon_message.body.set_auth_code(logon_details.auth_code);
    logon_message.body.set_two_factor_code(logon_details.two_auth_code);
    logon_message.body.set_login_key(logon_details.login_key);
    logon_message
}
