use serde_derive::Deserialize;
use serde_derive::Serialize;
use steam_language_gen::generated::enums::EResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewAPIKeyRequest {
    #[serde(rename = "agreeToTerms")]
    pub agree_to_terms: &'static str,
    /// API Key Name
    pub domain: &'static str,
    /// Requires confirming through Steam Mobile Authenticator
    pub request_id: String,
    #[serde(rename = "sessionid")]
    pub session_id: String,
}

impl NewAPIKeyRequest {
    pub(crate) fn new(request_id: String, session_id: String) -> Self {
        Self {
            agree_to_terms: "true",
            domain: "steam-mobile",
            request_id,
            session_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewAPIKeyResponse {
    pub success: EResult,
    /// Requires confirming through Steam Mobile Authenticator
    pub requires_confirmation: i64,
    pub api_key: Option<String>,
    pub request_id: Option<String>,
}
