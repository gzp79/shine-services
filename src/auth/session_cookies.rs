use serde::{Deserialize, Serialize};
use shine_service::{
    axum::session::{Session, SessionMeta},
    service::{serde_session_key, SessionKey, UserSession},
};
use uuid::Uuid;

/// Session information of a user.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub(in crate::auth) struct SessionData {
    #[serde(rename = "id")]
    pub user_id: Uuid,
    #[serde(rename = "sid", with = "serde_session_key")]
    pub key: SessionKey,
}

impl From<UserSession> for SessionData {
    fn from(value: UserSession) -> Self {
        Self {
            user_id: value.user_id,
            key: value.key,
        }
    }
}

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
        link_session_id: Option<SessionData>,
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

pub(in crate::auth) type AppSessionMeta = SessionMeta<SessionData>;
pub(in crate::auth) type AppSession = Session<SessionData>;
pub(in crate::auth) type ExternalLoginMeta = SessionMeta<ExternalLoginData>;
pub(in crate::auth) type ExternalLoginSession = Session<ExternalLoginData>;
