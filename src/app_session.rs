use crate::db::{serde_opt_session_id, serde_session_id, SessionId};
use serde::{Deserialize, Serialize};
use shine_service::axum::session::{Session, SessionMeta};
use uuid::Uuid;

/// Session information of a user.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SessionData {
    #[serde(rename = "id")]
    pub user_id: Uuid,
    #[serde(rename = "sid", with = "serde_session_id")]
    pub session_id: SessionId,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ExternalLoginData {
    #[serde(rename = "oid")]
    OIDCLogin {
        #[serde(rename = "pv")]
        pkce_code_verifier: String,
        #[serde(rename = "cv")]
        csrf_state: String,
        #[serde(rename = "n")]
        nonce: String,
        #[serde(rename = "u")]
        redirect_url: Option<String>,
        // indicates if login was made to link the account to the user of the given session
        #[serde(rename = "l", with = "serde_opt_session_id")]
        link_session_id: Option<SessionId>,
    },
}

impl std::fmt::Debug for ExternalLoginData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OIDCLogin {
                redirect_url,
                link_session_id,
                ..
            } => f
                .debug_struct("OIDCLogin")
                .field("pkce_code_verifier", &"[REDACTED]")
                .field("csrf_state", &"[REDACTED]")
                .field("nonce", &"[REDACTED]")
                .field("redirect_url", &redirect_url)
                .field("link_session", &link_session_id)
                .finish(),
        }
    }
}

pub type AppSessionMeta = SessionMeta<SessionData>;
pub type AppSession = Session<SessionData>;
pub type ExternalLoginMeta = SessionMeta<ExternalLoginData>;
pub type ExternalLoginSession = Session<ExternalLoginData>;
