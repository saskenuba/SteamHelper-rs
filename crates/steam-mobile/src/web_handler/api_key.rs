use serde_derive::Deserialize;
use serde_derive::Serialize;

use crate::types::ReceivedEResult;

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

/// Revokes the account's current Steam Web API key.
///
/// Steam exposes this as the classic form POST to `/dev/revokekey`. The `revoke` field mirrors the submit button's
/// value, which Steam still expects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokeAPIKeyRequest {
    #[serde(rename = "Revoke")]
    pub revoke: &'static str,
    #[serde(rename = "sessionid")]
    pub session_id: String,
}

impl RevokeAPIKeyRequest {
    pub(crate) fn new(session_id: String) -> Self {
        Self {
            revoke: "Revoke My Steam Web API Key",
            session_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewAPIKeyResponse {
    pub success: ReceivedEResult,
    /// Requires confirming through Steam Mobile Authenticator
    pub requires_confirmation: i64,
    pub api_key: Option<String>,
    pub request_id: Option<String>,
}
