use serde::{Deserialize, Serialize};
use shine_service::axum::session::{Session, SessionMeta};

#[derive(Clone, Serialize, Deserialize)]
pub enum SessionData {
    #[serde(rename = "gl")]
    GoogleLogin {
        #[serde(rename = "pv")]
        pkce_code_verifier: String,
        #[serde(rename = "cv")]
        csrf_state: String,        
        #[serde(rename = "u")]
        redirect_url: Option<String>,
    },
}

impl std::fmt::Debug for SessionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GoogleLogin { redirect_url, .. } => f
                .debug_struct("GoogleLogin")
                .field("pkce_code_verifier", &"[REDACTED]")
                .field("redirect_url", redirect_url)
                .finish(),
        }
    }
}

pub type AppSessionMeta = SessionMeta<SessionData>;
pub type AppSession = Session<SessionData>;
