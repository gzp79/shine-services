use crate::{app_state::AppState, repositories::identity::TokenKind};
use shine_core::web::CurrentUser;

pub struct SessionUtils<'a> {
    app_state: &'a AppState,
}

impl<'a> SessionUtils<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self { app_state }
    }

    pub async fn revoke_session(&self, user_session: CurrentUser) {
        if let Err(err) = self
            .app_state
            .session_service()
            .remove(user_session.user_id, &user_session.key)
            .await
        {
            log::error!("Failed to revoke session for user {}: {}", user_session.user_id, err);
        }
    }

    pub async fn revoke_opt_session(&self, user_session: Option<CurrentUser>) {
        if let Some(user_session) = user_session {
            self.revoke_session(user_session).await;
        }
    }

    pub async fn revoke_access(&self, kind: TokenKind, token: &str) {
        if let Err(err) = self.app_state.identity_service().delete_token(kind, token).await {
            log::error!("Failed to revoke ({:?}) token ({}): {}", kind, token, err);
        }
    }

    pub async fn revoke_opt_access(&self, kind: TokenKind, token: Option<String>) {
        if let Some(revoked_token) = token {
            self.revoke_access(kind, &revoked_token).await;
        }
    }
}
