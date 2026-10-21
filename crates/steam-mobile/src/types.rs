use std::borrow::Cow;

use serde::de::value::I32Deserializer;
use serde::de::IntoDeserializer;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde_with::serde_as;
use serde_with::Map;
use steam_language::enums::EResult;
use tracing::warn;

use crate::web_handler::confirmation::Confirmation;
use crate::STEAM_COMMUNITY_BASE;

/// Used to login into Steam website if it detects something different on your account.
/// This may be because of unsuccessful logins, numerous retries on some operations. or anything. Really.
/// Requiring a captcha certainly can occur on an account with SteamGuard disabled, but still not sure on an account
/// with it disabled.
///
/// The safe way to use this if you are automating something, is to match the error and perhaps have a captcha solver.
/// You can get the captcha GUID if it is required through the `LoginError::CaptchaRequired`.
#[derive(Serialize, Debug, Clone)]
pub struct LoginCaptcha<'a> {
    #[serde(rename = "captcha_gid")]
    /// Captcha GUID. I.e unique identifier.
    pub guid: &'a str,
    #[serde(rename = "captcha_text")]
    /// Text transcription of captcha,
    pub text: &'a str,
}

#[derive(Serialize, Debug, Clone)]
pub struct ConfirmationBase<'a> {
    #[serde(rename = "p")]
    pub device_id: Cow<'a, str>,
    #[serde(rename = "a")]
    pub steamid: Cow<'a, str>,
    #[serde(rename = "k")]
    pub confirmation_hash: Cow<'a, str>,
    #[serde(rename = "t")]
    pub time: Cow<'a, str>,
    #[serde(rename = "m")]
    pub device_kind: Cow<'a, str>,
    pub tag: Cow<'a, str>,
    #[serde(rename = "op")]
    pub operation: Option<Cow<'a, str>>,
}

#[serde_as]
#[derive(Serialize, Debug, Clone)]
pub struct ConfirmationMultiAcceptRequest<'a> {
    #[serde(flatten)]
    pub base: ConfirmationBase<'a>,

    #[serde(flatten)]
    #[serde_as(as = "Map<_, _>")]
    pub confirmation_id: Vec<(&'static str, Cow<'a, str>)>,
    #[serde(flatten)]
    #[serde_as(as = "Map<_, _>")]
    pub confirmation_key: Vec<(&'static str, Cow<'a, str>)>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConfirmationResponseBase {
    success: bool,
    pub conf: Vec<Confirmation>,
}

/// The [`EResult`] with this number, if steam-language knows it.
pub(crate) fn eresult_from_i32(value: i32) -> Option<EResult> {
    let deserializer: I32Deserializer<serde::de::value::Error> = value.into_deserializer();
    EResult::deserialize(deserializer).ok()
}

/// An [`EResult`] as Steam sent it. A number steam-language doesn't know yet reads as [`EResult::Invalid`] instead of
/// failing the whole response, as [`crate::EConfirmationType`] does with its `Unknown`; `raw` keeps the number for
/// error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReceivedEResult {
    pub(crate) eresult: EResult,
    pub(crate) raw: i32,
}

impl PartialEq<EResult> for ReceivedEResult {
    fn eq(&self, other: &EResult) -> bool {
        self.eresult == *other
    }
}

/// `AccessDenied (15)`, or `unknown (100000)`.
impl std::fmt::Display for ReceivedEResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match eresult_from_i32(self.raw) {
            Some(known) => write!(f, "{known:?} ({})", self.raw),
            None => write!(f, "unknown ({})", self.raw),
        }
    }
}

impl<'de> Deserialize<'de> for ReceivedEResult {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = i32::deserialize(deserializer)?;
        let eresult = eresult_from_i32(raw).unwrap_or_else(|| {
            warn!("Steam sent an unknown EResult {raw}, read as Invalid.");
            EResult::Invalid
        });
        Ok(Self { eresult, raw })
    }
}

impl Serialize for ReceivedEResult {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(self.raw)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ParentalUnlockResponse {
    pub success: bool,
    pub eresult: ReceivedEResult,
    pub error_message: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BooleanResponse {
    pub success: bool,
}

/// No `Debug`: holds the parental PIN.
#[derive(Serialize, Clone)]
pub struct ParentalUnlockRequest<'a> {
    /// Parental Unlock Code
    pub pin: &'a str,
    pub sessionid: &'a str,
}

#[derive(Serialize, Debug, Clone)]
pub struct IEconServiceGetTradeOffersRequest {
    pub active_only: u8,
    pub get_descriptions: u8,
    pub get_sent_offers: u8,
    pub get_received_offers: u8,
    #[serde(rename = "key")]
    pub api_key: String,
    pub time_historical_cutoff: u32,
}

impl Default for IEconServiceGetTradeOffersRequest {
    fn default() -> Self {
        Self {
            active_only: 1,
            get_descriptions: 1,
            get_received_offers: 1,
            get_sent_offers: 0,
            api_key: "".to_string(),
            time_historical_cutoff: u32::max_value(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginErrorGenericMessage<'a> {
    pub success: bool,
    pub message: Cow<'a, str>,
}

#[allow(non_camel_case_types)]
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RSAResponseBase {
    #[serde(rename = "response")]
    pub inner: RSAResponse,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RSAResponse {
    pub publickey_mod: String,
    pub publickey_exp: String,
    pub timestamp: String,
}

#[derive(Debug, Deserialize)]
struct AjaxRefreshResponse {
    success: bool,
    login_url: String, // settoken url
    #[serde(rename = "steamID")]
    steam_id: String,
    nonce: String,
    redit: String,
    auth: String,
}

/// Request containing data coming from AjaxRefresh
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SetTokenRequest {
    #[serde(rename = "steamID")]
    steam_id: String,
    nonce: String,
    redir: String,
    auth: String,
}

#[derive(Debug, Deserialize)]
struct SetTokenResponse {
    response: i32, // 1 if ok
}

/// No `Debug`: `nonce` is the refresh token.
#[derive(Serialize)]
pub struct FinalizeLoginRequest {
    nonce: String,
    #[serde(rename = "sessionid")]
    session_id: String,
    redir: String,
}

impl FinalizeLoginRequest {
    pub(crate) fn new(refresh_token: String, session_id: String) -> Self {
        Self {
            nonce: refresh_token,
            session_id,
            redir: STEAM_COMMUNITY_BASE.to_owned() + "/login/home?goto=",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FinalizeLoginResponseBase {
    #[serde(rename = "steamID")]
    pub(crate) steam_id: String,
    redir: String,
    #[serde(rename = "transfer_info")]
    pub(crate) domain_tokens: Vec<DomainToken>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainToken {
    /// One of Steam's domains to ping and receive cookies for.
    pub url: String,
    pub params: DomainTokenData,
}

/// Contains tokens to authenticate into a Steam domain.
#[derive(Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainTokenData {
    pub nonce: String,
    pub auth: String,
    #[serde(rename = "steamID")]
    pub steam_id: Option<String>,
}

impl std::fmt::Debug for DomainTokenData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DomainTokenData")
            .field("steam_id", &self.steam_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_handler::api_key::NewAPIKeyResponse;

    #[test]
    fn known_eresults_parse_as_before() {
        let response: ParentalUnlockResponse =
            serde_json::from_str(r#"{"success":false,"eresult":15,"error_message":false}"#).unwrap();
        assert_eq!(response.eresult, EResult::AccessDenied);
        assert_eq!(response.eresult.to_string(), "AccessDenied (15)");

        let response: NewAPIKeyResponse =
            serde_json::from_str(r#"{"success":1,"requires_confirmation":0,"api_key":"key"}"#).unwrap();
        assert_eq!(response.success, EResult::OK);
        assert_eq!(
            serde_json::to_string(&response).unwrap(),
            r#"{"success":1,"requires_confirmation":0,"api_key":"key","request_id":null}"#
        );

        assert_eq!(eresult_from_i32(1), Some(EResult::OK));
        assert_eq!(eresult_from_i32(0), Some(EResult::Invalid));
    }

    #[test]
    fn unknown_eresults_read_as_invalid() {
        assert_eq!(eresult_from_i32(100_000), None);

        let response: ParentalUnlockResponse =
            serde_json::from_str(r#"{"success":false,"eresult":100000,"error_message":false}"#).unwrap();
        assert_eq!(response.eresult, EResult::Invalid);
        assert_eq!(response.eresult.raw, 100_000);
        assert_eq!(response.eresult.to_string(), "unknown (100000)");

        let response: NewAPIKeyResponse =
            serde_json::from_str(r#"{"success":100000,"requires_confirmation":0}"#).unwrap();
        assert_eq!(response.success, EResult::Invalid);
        assert_eq!(response.success.to_string(), "unknown (100000)");
    }

    #[test]
    fn eresults_that_are_not_numbers_still_fail() {
        assert!(serde_json::from_str::<NewAPIKeyResponse>(r#"{"success":"1","requires_confirmation":0}"#).is_err());
        assert!(serde_json::from_str::<NewAPIKeyResponse>(r#"{"success":1.5,"requires_confirmation":0}"#).is_err());
    }
}
