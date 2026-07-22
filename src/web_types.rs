use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct RegisterRuntimeRequest {
    pub access_token: String,
    pub agent_public_key: String,
}

impl fmt::Debug for RegisterRuntimeRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegisterRuntimeRequest")
            .field("access_token", &"[REDACTED]")
            .field("agent_public_key", &self.agent_public_key)
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RegisterRuntimeResponse {
    pub agent_runtime_id: String,
    pub account: RuntimeAccount,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RuntimeAccount {
    pub account_id: String,
    pub chatgpt_user_id: String,
    pub email: String,
    pub plan_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ApiErrorEnvelope {
    pub error: ApiErrorBody,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ApiErrorBody {
    pub code: ApiErrorCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiErrorCode {
    InvalidRequest,
    InvalidToken,
    TokenExpired,
    RegistrationRejected,
    UpstreamUnavailable,
    InternalError,
}

impl ApiErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::InvalidToken => "INVALID_TOKEN",
            Self::TokenExpired => "TOKEN_EXPIRED",
            Self::RegistrationRejected => "REGISTRATION_REJECTED",
            Self::UpstreamUnavailable => "UPSTREAM_UNAVAILABLE",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_debug_output_redacts_the_access_token() {
        let token = "header.payload.signature";
        let request = RegisterRuntimeRequest {
            access_token: token.to_owned(),
            agent_public_key: "ssh-ed25519 public-key".to_owned(),
        };

        let debug = format!("{request:?}");

        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains(token));
    }
}
