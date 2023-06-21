use serde::{Deserialize, Serialize};
use shine_service::{
    axum::session::{Session, SessionMeta},
    service::UserSessionData,
};

#[derive(Clone, Serialize, Deserialize)]
pub(in crate::auth) enum ExternalLoginData {
    #[serde(rename = "oid")]
    OIDCLogin {
        #[serde(rename = "pv")]
        pkce_code_verifier: String,
        #[serde(rename = "cv")]
        csrf_state: String,
        #[serde(rename = "n")]
        nonce: String,
        #[serde(rename = "t")]
        target_url: Option<String>,
        // indicates if login was made to link the account to the user of the given session
        #[serde(rename = "l")]
        link_session_id: Option<UserSessionData>,
    },
}

impl std::fmt::Debug for ExternalLoginData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OIDCLogin {
                target_url,
                link_session_id,
                ..
            } => f
                .debug_struct("OIDCLogin")
                .field("pkce_code_verifier", &"[REDACTED]")
                .field("csrf_state", &"[REDACTED]")
                .field("nonce", &"[REDACTED]")
                .field("target_url", &target_url)
                .field("link_session", &link_session_id)
                .finish(),
        }
    }
}

pub(in crate::auth) type ExternalLoginMeta = SessionMeta<ExternalLoginData>;
pub(in crate::auth) type ExternalLoginSession = Session<ExternalLoginData>;
