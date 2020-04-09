use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct RSAResponse {
    success: bool,
    #[serde(rename = "publickey_exp")]
    pub(crate) exponent: String,
    #[serde(rename = "publickey_mod")]
    pub(crate) modulus: String,
    pub(crate) timestamp: String,
    token_gid: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest<'a> {
    pub donotcache: &'a str,
    pub password: &'a str,
    pub username: &'a str,
    pub twofactorcode: &'a str,
    pub emailauth: String,
    pub loginfriendlyname: String,
    pub captchagid: &'a str,
    pub captcha_text: &'a str,
    pub emailsteamid: String,
    pub rsatimestamp: String,
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
            emailauth: "".to_string(),
            loginfriendlyname: "".to_string(),
            captchagid: "-1",
            captcha_text: "",
            emailsteamid: "".to_string(),
            rsatimestamp: "".to_string(),
            remember_login: "false",
            oauth_client_id: "DE45CD61",
            oauth_score: "read_profile write_profile read_client write_client",
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub success: bool,
    #[serde(rename = "requires_twofactor")]
    pub requires_twofactor: bool,
    #[serde(rename = "login_complete")]
    pub login_complete: bool,
    #[serde(rename = "transfer_urls")]
    pub transfer_urls: Vec<String>,
    #[serde(rename = "transfer_parameters")]
    pub transfer_parameters: TransferParameters,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferParameters {
    pub steamid: String,
    pub token_secure: String,
    pub auth: String,
    pub remember_login: bool,
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
