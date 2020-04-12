use crate::SteamAuthenticator;
use serde::{Deserialize, Serialize};

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
    pub oauth_token: String,
    pub wgtoken: String,
    pub wgtoken_secure: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddAuthenticator {
    pub access_token: String,
    pub steamid: String,
    pub authenticator_type: String,
    pub device_identifier: String,
    pub sms_phone_id: String,
}

impl Default for AddAuthenticator {
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