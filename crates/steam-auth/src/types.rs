use serde::{Deserialize, Serialize};
use steam_language_gen::generated::enums::{ETradeOfferState, ETradeOfferConfirmationMethod};


#[derive(Serialize, Debug, Clone)]
pub struct ConfirmationMultiAcceptRequest<'a> {
    #[serde(rename = "a")]
    pub steamid: String,
    #[serde(rename = "k")]
    pub confirmation_hash: String,
    #[serde(rename = "m")]
    pub device_kind: String,
    #[serde(rename = "op")]
    /// Accept or cancel confirmation
    pub operation: String,
    #[serde(rename = "p")]
    pub device_id: String,
    #[serde(rename = "t")]
    pub time: &'a str,
    pub tag: &'a str,
    #[serde(flatten)]
    pub confirmation: Vec<ConfirmationParameter>
}

#[derive(Serialize, Debug, Clone)]
pub struct ConfirmationParameter {
    #[serde(rename = "cid[]")]
    pub confirmation_id: String,
    #[serde(rename = "ck[]")]
    pub confirmation_key: String
}


#[derive(Deserialize, Debug, Clone)]
pub struct ConfirmationDetailsResponse {
    success: bool,
    pub html: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ParentalUnlockResponse {
    pub success: bool,
    pub eresult: u32,
    pub error_message: bool,
}

#[derive(Serialize, Debug, Clone)]
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

#[derive(Serialize, Debug, Clone)]
pub struct PhoneAjaxRequest<'a> {
    #[serde(rename = "op")]
    pub operation: &'a str,
    #[serde(rename = "arg")]
    pub operation_arg: &'a str,
    pub sessionid: &'a str,
}

#[derive(Serialize, Debug, Clone)]
pub struct CheckSmsCodeRequest<'a> {
    pub skipvoip: &'a str,
    pub checkfortos: &'a str,
    #[serde(flatten)]
    pub ops: PhoneAjaxRequest<'a>,
}

impl<'a> Default for CheckSmsCodeRequest<'a> {
    fn default() -> Self {
        Self {
            skipvoip: "1",
            checkfortos: "0",
            ops: PhoneAjaxRequest { operation: "check_sms_code", operation_arg: "", sessionid: "" },
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RSAResponse {
    success: bool,
    #[serde(rename = "publickey_exp")]
    pub exponent: String,
    #[serde(rename = "publickey_mod")]
    pub modulus: String,
    pub timestamp: String,
    token_gid: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginRequest<'a> {
    pub donotcache: &'a str,
    pub password: &'a str,
    pub username: &'a str,
    pub twofactorcode: &'a str,
    pub emailauth: &'a str,
    pub loginfriendlyname: &'a str,
    #[serde(rename = "captchagid")]
    pub captcha_gid: &'a str,
    pub captcha_text: &'a str,
    pub emailsteamid: &'a str,
    #[serde(rename = "rsatimestamp")]
    pub rsa_timestamp: String,
    pub remember_login: &'a str,
    pub oauth_client_id: &'a str,
    pub oauth_score: &'a str,
}

impl<'a> Default for LoginRequest<'a> {
    fn default() -> Self {
        Self {
            donotcache: "",
            password: "",
            username: "",
            twofactorcode: "",
            emailauth: "",
            loginfriendlyname: "",
            captcha_gid: "-1",
            captcha_text: "",
            emailsteamid: "",
            rsa_timestamp: "".to_string(),
            remember_login: "false",
            oauth_client_id: "DE45CD61",
            oauth_score: "read_profile write_profile read_client write_client",
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    pub requires_twofactor: bool,
    pub login_complete: bool,
    pub transfer_urls: Vec<String>,
    pub transfer_parameters: TransferParameters,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct TransferParameters {
    pub steamid: String,
    pub token_secure: String,
    pub auth: String,
    pub remember_login: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct LoginResponseMobile {
    pub success: bool,
    pub requires_twofactor: bool,
    pub redirect_uri: String,
    pub login_complete: bool,
    #[serde(deserialize_with = "serde_with::json::nested::deserialize")]
    pub oauth: Oauth,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct Oauth {
    pub steamid: String,
    pub account_name: String,
    /// This is also knwon as "access_token", and can be used to refresh sessions.
    pub oauth_token: String,
    pub wgtoken: String,
    pub wgtoken_secure: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FinalizeAddAuthenticatorBase {
    pub response: FinalizeAddAuthenticatorResponse,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FinalizeAddAuthenticatorResponse {
    pub status: String,
    pub server_time: String,
    pub want_more: String,
    pub success: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FinalizeAddAuthenticatorRequest<'a> {
    pub access_token: &'a str,
    pub activation_code: &'a str,
    pub authenticator_code: &'a str,
    pub authenticator_time: &'a str,
    pub steamid: &'a str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AddAuthenticatorRequest {
    pub access_token: String,
    pub steamid: String,
    pub authenticator_type: String,
    pub device_identifier: String,
    pub sms_phone_id: String,
}

impl Default for AddAuthenticatorRequest {
    fn default() -> Self {
        Self {
            access_token: "".to_string(),
            steamid: "".to_string(),
            authenticator_type: "1".to_string(),
            device_identifier: "".to_string(),
            sms_phone_id: "1".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RemoveAuthenticatorRequest<'a> {
    pub steamid: &'a str,
    pub steamguard_scheme: &'a str,
    pub revocation_code: &'a str,
    /// This is also known as the oauth token. We receive it after the mobile logOn.
    pub access_token: &'a str,
}

impl<'a> Default for RemoveAuthenticatorRequest<'a> {
    fn default() -> Self {
        Self { steamid: "", steamguard_scheme: "2", revocation_code: "", access_token: "" }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiKeyRegisterRequest<'a> {
    #[serde(rename = "agreeToTerms")]
    agree_to_terms: &'a str,
    domain: &'a str,
    #[serde(rename = "Submit")]
    submit: &'a str,
}

impl<'a> Default for ApiKeyRegisterRequest<'a> {
    fn default() -> Self {
        Self { agree_to_terms: "agreed", domain: "localhost", submit: "Register" }
    }
}

#[derive(Deserialize)]
pub struct ISteamUserAuthResponse {
    token: String,
    #[serde(rename = "tokensecure")]
    token_secure: String,
}

#[derive(Serialize)]
pub struct ISteamUserAuthRequest {
    pub steamid: String,
    #[serde(rename = "sessionkey")]
    pub session_key: String,
    pub encrypted_loginkey: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolveVanityUrlBaseResponse {
    pub response: ResolveVanityUrlResponse,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolveVanityUrlResponse {
    pub steamid: String,
    pub success: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolveVanityUrlRequest {
    #[serde(rename = "key")]
    api_key: String,
    #[serde(rename = "vanityurl")]
    vanity_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct IEconServiceGetTradeOffersResponse {
    pub trade_offers_sent: Vec<IEconTradeOffer>,
    pub trade_offers_received: Vec<IEconTradeOffer>,
    pub descriptions: Descriptions,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Descriptions {
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub appid: u32,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub classid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub instanceid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub marketable: bool,
    pub tradable: String,
}

/// Represents a steam trade offer.
/// Returned by GetTradeOffers (vector) and GetTradeOffer.
#[derive(Deserialize, Debug, Clone)]
pub struct IEconTradeOffer {
    /// Unique ID generated when a trade offer is created
    #[serde(with = "serde_with::rust::display_fromstr")]
    tradeofferid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    /// the other steamid in the format of steamid3?
    accountid_other: u32,
    /// Message included by the creator of the trade offer
    message: String,
    expiration_time: String,
    /// State of trade offer
    trade_offer_state: ETradeOfferState,
    items_to_give: String,
    items_to_receive: String,
    /// Indicates the account binded with the api key requested this trade
    #[serde(with = "serde_with::rust::display_fromstr")]
    is_our_offer: bool,
    time_created: String,
    time_updated: String,
    from_real_time_trade: String,
    escrow_end_date: String,
    confirmation_method: ETradeOfferConfirmationMethod,
}
