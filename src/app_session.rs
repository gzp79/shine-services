use serde::{Deserialize, Serialize};
use shine_service::axum::session::{Session, SessionMeta};
use sqlx::types::Uuid;

/// Session information of a user.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SessionData {
    #[serde(rename = "id")]
    pub id: Uuid,
    #[serde(rename = "sid")]
    pub session_id: String,
}

/// External login information:
/// - creating a new account
/// - linking an existing account
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ExternalLoginData {
    /// The id of the session to link this external provider to. If this is a normal login,
    /// this member should be None.
    #[serde(rename = "sig")]
    pub session_id: Option<String>,
    #[serde(rename = "st")]
    pub state: ExternalLoginState,

}

#[derive(Clone, Serialize, Deserialize)]
pub enum ExternalLoginState {
    #[serde(rename = "oid")]
    OpenIdConnectLogin {
        #[serde(rename = "pv")]
        pkce_code_verifier: String,
        #[serde(rename = "cv")]
        csrf_state: String,
        #[serde(rename = "n")]
        nonce: String,        
        #[serde(rename = "u")]
        redirect_url: Option<String>,
    },
}

impl std::fmt::Debug for ExternalLoginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenIdConnectLogin { redirect_url, .. } => f
                .debug_struct("OpenIdConnectLogin")
                .field("pkce_code_verifier", &"[REDACTED]")
                .field("redirect_url", redirect_url)
                .finish(),
        }
    }
}

pub type AppSessionMeta = SessionMeta<SessionData>;
pub type AppSession = Session<SessionData>;
pub type ExternalLoginMeta = SessionMeta<ExternalLoginData>;
pub type ExternalLoginSession = Session<ExternalLoginData>;
