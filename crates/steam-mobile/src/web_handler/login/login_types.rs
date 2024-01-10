//! Types used to login into Steam via web

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct PollAuthSessionStatusRequest {
    pub client_id: String,
    pub request_id: String,
}

impl PollAuthSessionStatusRequest {
    pub fn new(client_id: String, request_id: String) -> Self {
        Self { client_id, request_id }
    }

    pub fn from_begin_auth_response(response: BeginAuthSessionViaCredentialsResponse) -> Self {
        Self::new(response.client_id, response.request_id)
    }
}

#[derive(Debug, Deserialize)]
pub struct PollAuthSessionStatusResponseBase {
    #[serde(rename = "response")]
    pub inner: PollAuthSessionStatusResponse,
}

#[derive(Debug, Deserialize)]
pub struct PollAuthSessionStatusResponse {
    pub refresh_token: Option<String>,
    pub access_token: Option<String>,
    pub account_name: Option<String>,
    pub had_remote_interaction: Option<bool>,
}

// FIXME: how to trigger captcha?
#[derive(Debug, Serialize)]
pub struct BeginAuthSessionViaCredentialsRequest {
    #[serde(rename = "account_name")]
    pub username: String,
    pub encrypted_password: String,
    pub encryption_timestamp: String,
    pub persistence: String,
    device_details: Option<String>,
    device_friendly_name: Option<String>,
    guard_data: Option<String>,
    language: Option<u32>,
    platform_type: Option<String>,
}

impl BeginAuthSessionViaCredentialsRequest {
    /// # Arguments
    ///
    /// * `username`: Steam username.
    /// * `encrypted_password`: Steam's password encrypted by Steam retrieved account RSA public key and encoded to b64.
    /// * `encrypted_password_timestamp`: Timestamp representing when password was encrypted. Returned together with RSA
    ///   public key.
    ///
    /// returns: [BeginAuthSessionViaCredentialsRequest]
    ///
    /// # Examples
    ///
    /// ```
    /// ```
    pub fn new(username: String, encrypted_password: String, encrypted_password_timestamp: String) -> Self {
        Self {
            username,
            encrypted_password,
            encryption_timestamp: encrypted_password_timestamp,
            persistence: "1".to_string(),
            device_details: None,
            device_friendly_name: None,
            guard_data: None,
            language: None,
            platform_type: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BeginAuthSessionViaCredentialsResponseBase {
    #[serde(rename = "response")]
    pub(crate) inner: BeginAuthSessionViaCredentialsResponse,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BeginAuthSessionViaCredentialsResponse {
    pub client_id: String,
    /// Base 64 String
    pub request_id: String,
    interval: u32,
    #[serde(rename = "steamid")]
    steam_id: String,
    extended_error_message: Option<String>,
    /// [ "confirmation_type": 3 ]
    pub allowed_confirmations: Vec<AllowedConfirmations>,
    captcha_needed: Option<bool>,
    /// JWT token of some kind
    weak_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AllowedConfirmations {
    pub confirmation_type: u32,
}

/// Response for this request will be \"response: {}"
#[derive(Debug, Clone, Serialize)]
pub struct UpdateAuthSessionWithSteamGuardCodeRequest {
    pub client_id: String,
    #[serde(rename = "steamid")]
    steam_id: String,
    code: String,
    /// 2 for email codes, 3 for steam guard mobile
    code_type: u32,
}

impl UpdateAuthSessionWithSteamGuardCodeRequest {
    pub fn from_begin_auth_response(
        response: BeginAuthSessionViaCredentialsResponseBase,
        steam_guard_code: String,
    ) -> Self {
        Self {
            client_id: response.inner.client_id,
            steam_id: response.inner.steam_id,
            code: steam_guard_code,
            //
            code_type: 3,
        }
    }
}
