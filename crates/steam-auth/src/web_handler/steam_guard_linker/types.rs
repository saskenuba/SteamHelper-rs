use serde::{Deserialize, Serialize};
use strum_macros::{Display, IntoStaticStr};

use crate::utils::generate_canonical_device_id;
use crate::MobileAuthFile;

#[derive(Copy, Clone, Debug, Serialize, Display, IntoStaticStr)]
pub enum PhoneAjaxOperation {
    #[strum(serialize = "check_sms_code")]
    CheckSMSCode,
    #[strum(serialize = "add_phone_number")]
    AddPhoneNumber,
    #[strum(serialize = "email_confirmation")]
    EmailConfirmation,
    #[strum(serialize = "has_phone")]
    HasPhone,
}

#[derive(Copy, Clone, Debug, Serialize, Display, IntoStaticStr)]
/// This is finite state machine.
/// We get the future state as a response of the current state.
///
///
/// Example: If the current operation 'retry_email_verification' succeds, we will receive
/// 'get_sms_code' on the state field of the response, indicating that it will be the next operation.
pub enum StorePhoneAjaxOp {
    #[strum(serialize = "retry_email_verification")]
    RetryEmailVerification,
    #[strum(serialize = "get_sms_code")]
    GetSMSCode,
    #[strum(serialize = "done")]
    Done,
}

#[derive(Debug, Serialize)]
pub struct PhoneAjaxRequest<'a> {
    #[serde(rename = "op")]
    operation: &'static str,
    #[serde(rename = "arg")]
    argument: &'a str,
    sessionid: String,
    #[serde(flatten)]
    operation_related: Option<PhoneOptions>,
}

impl<'a> PhoneAjaxRequest<'a> {
    pub fn add_phone(session_id: &'a str, phone_number: &'a str) -> PhoneAjaxRequest<'a> {
        let params = PhoneOptions {
            checkfortos: "0",
            skipvoip: "0",
        };

        Self {
            operation: PhoneAjaxOperation::AddPhoneNumber.into(),
            argument: phone_number,
            sessionid: session_id.to_string(),
            operation_related: Some(params),
        }
    }

    pub fn check_email_confirmation(session_id: &str) -> PhoneAjaxRequest<'a> {
        Self {
            operation: PhoneAjaxOperation::EmailConfirmation.into(),
            argument: "",
            sessionid: session_id.to_string(),
            operation_related: None,
        }
    }

    pub fn has_phone(session_id: &str) -> PhoneAjaxRequest<'a> {
        let params = PhoneOptions {
            checkfortos: "0",
            skipvoip: "1",
        };

        Self {
            operation: PhoneAjaxOperation::HasPhone.into(),
            argument: "null",
            sessionid: session_id.to_string(),
            operation_related: Some(params),
        }
    }

    pub fn check_sms(session_id: &'a str, sms_code: &'a str) -> PhoneAjaxRequest<'a> {
        let params = PhoneOptions {
            checkfortos: "0",
            skipvoip: "1",
        };

        Self {
            operation: PhoneAjaxOperation::CheckSMSCode.into(),
            argument: sms_code,
            sessionid: session_id.to_string(),
            operation_related: Some(params),
        }
    }
}

#[derive(Debug, Serialize)]
struct PhoneOptions {
    checkfortos: &'static str,
    skipvoip: &'static str,
}

#[derive(Debug, Deserialize)]
pub(super) struct HasPhoneResponse {
    #[serde(rename = "has_phone")]
    pub user_has_phone: bool,
}

#[derive(Debug, Deserialize)]
/// Used for AddPhoneNumber and CheckEmailConfirmation.
pub(super) struct GenericSuccessResponse {
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RemoveAuthenticatorRequest<'a> {
    pub steamid: &'a str,
    pub revocation_code: &'a str,
    #[serde(rename = "access_token")]
    pub oauth_token: &'a str,
    pub steamguard_scheme: &'a str,
}

impl<'a> Default for RemoveAuthenticatorRequest<'a> {
    fn default() -> Self {
        Self {
            steamid: "",
            revocation_code: "",
            oauth_token: "",
            steamguard_scheme: "2",
        }
    }
}

#[derive(Default, Debug, Serialize)]
pub(super) struct FinalizeAddAuthenticatorRequest<'a> {
    pub steamid: &'a str,
    #[serde(rename = "access_token")]
    /// Oauth recovered from login
    pub oauth_token: &'a str,
    pub authenticator_code: String,
    pub authenticator_time: u64,
    #[serde(rename = "activation_code")]
    /// The SMS Confirmation Code received on phone number after the Add Authenticator step.
    pub sms_activation_code: &'a str,
}

impl<'a> FinalizeAddAuthenticatorRequest<'a> {
    pub fn swap_codes(&mut self, auth_code: String, auth_time: u64) {
        self.authenticator_code = auth_code;
        self.authenticator_time = auth_time;
    }
}

#[derive(Debug, Serialize)]
pub(super) struct AddAuthenticatorRequest<'a> {
    #[serde(rename = "access_token")]
    /// Oauth recovered from login
    oauth_token: &'a str,
    steamid: String,
    /// The automatically generated device_id based on user's SteamID.
    device_identifier: String,
    authenticator_time: u64,
    authenticator_type: u64,
    sms_phone_id: &'a str,
}

impl<'a> AddAuthenticatorRequest<'a> {
    pub fn new(oauth_token: &'a str, steamid: &str, steam_server_time: u64) -> Self {
        Self {
            oauth_token,
            device_identifier: generate_canonical_device_id(steamid),
            authenticator_time: steam_server_time,
            steamid: steamid.to_string(),
            ..Default::default()
        }
    }
}

impl<'a> Default for AddAuthenticatorRequest<'a> {
    fn default() -> Self {
        Self {
            oauth_token: "",
            steamid: "".to_string(),
            device_identifier: "".to_string(),
            authenticator_time: 0,
            authenticator_type: 1,
            sms_phone_id: "1",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AddAuthenticatorResponseBase {
    #[serde(rename = "response")]
    pub steam_guard_success_details: AddAuthenticatorResponse,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct AddAuthenticatorResponse {
    #[serde(flatten)]
    pub mobile_auth: MobileAuthFile,

    pub status: i64,
    /* pub shared_secret: String,
     * pub revocation_code: String,
     * pub identity_secret: String,
     * pub account_name: String,
     * pub secret1: String,
     * Deprecated
     * pub serial_number: String,
     * pub server_time: String,
     * pub token_gid: String,
     * pub uri: String, */
}

#[derive(Debug, Deserialize)]
pub struct AddAuthenticatorErrorResponseBase {
    pub response: AddAuthenticatorErrorResponse,
}

#[derive(Debug, Deserialize)]
pub struct AddAuthenticatorErrorResponse {
    pub status: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FinalizeAddAuthenticatorBase {
    pub response: FinalizeAddAuthenticatorResponse,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FinalizeAddAuthenticatorResponse {
    pub status: i64,
    pub server_time: String,
    /// Apparently, this means that Steam wants us to generate more codes to ensure that the generator is working as
    /// intended.
    pub want_more: bool,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FinalizeAddAuthenticatorErrorBase {
    pub response: FinalizeAddAuthenticatorErrorResponse,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FinalizeAddAuthenticatorErrorResponse {
    pub status: i64,
}

/// operacao após receber o SMS
/// endpoint na store https://store.steampowered.com/phone/add_ajaxop
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StoreFinalizeAuthenticatorRequest {
    input: String,
    #[serde(rename = "sessionID")]
    sessionid: String,
    confirmed: String,
    checkfortos: String,
    bisediting: String,
    token: String,
}

impl Default for StoreFinalizeAuthenticatorRequest {
    fn default() -> Self {
        Self {
            input: "".to_string(),
            sessionid: "".to_string(),
            confirmed: "1".to_string(),
            checkfortos: "1".to_string(),
            bisediting: "0".to_string(),
            token: "0".to_string(),
        }
    }
}

// {"success":true,"showResend":false,"state":"done","errorText":"","token":"0","vac_policy":0,"tos_policy":2,"showDone"
// :true,"maxLength":"5"}

#[derive(Default, Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreFinalizeAuthenticatorResponse {
    pub success: bool,
    pub show_resend: bool,
    pub state: String,
    pub error_text: String,
    pub token: String,
    #[serde(rename = "vac_policy")]
    pub vac_policy: i64,
    #[serde(rename = "tos_policy")]
    pub tos_policy: i64,
    pub show_done: bool,
    pub max_length: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phoneajax_serialization() {
        let stringified: &'static str = PhoneAjaxOperation::HasPhone.into();
        assert_eq!(stringified, "has_phone");
    }
}
