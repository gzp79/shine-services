use serde::{Deserialize, Serialize};
use shine_service::axum::session::{Session, SessionMeta};

//todo: maybe Cookie storing the parameters for the OAuthFlow have to be split to allow linking of logged-in users
// Now the flow would invalidate the current user and unless we store it in the GoogleLogin variant we loose the
// target user. Also a failed link attempt would sign out the user (drop the cookie) which might not be a good thing.
// If they are separated the question is, if it may happen that the signed in user is altered during a linking flow and hence
// the linking happens to a wrong user.


#[derive(Clone, Serialize, Deserialize)]
pub enum SessionData {
    #[serde(rename = "gl")]
    GoogleLogin {
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
