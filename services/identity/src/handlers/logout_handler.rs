use crate::{
    app_state::AppState,
    handlers::{AuthPage, AuthPageHandler},
    models::TokenKind,
    repositories::{
        identity::{pg::PgIdentityDb, IdentityDb},
        session::{redis::RedisSessionDb, SessionDb},
    },
    routes::auth::AuthSession,
    services::{SessionService, TokenService},
};
use url::Url;

/// Handler for user logout
///
/// Orchestrates token revocation and session removal on logout.
pub struct LogoutHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    page_handler: AuthPageHandler<'a>,
    token_service: &'a TokenService<IDB>,
    session_service: &'a SessionService<SDB>,
}

impl<'a, IDB, SDB> LogoutHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    pub fn new(
        page_handler: AuthPageHandler<'a>,
        token_service: &'a TokenService<IDB>,
        session_service: &'a SessionService<SDB>,
    ) -> Self {
        Self {
            page_handler,
            token_service,
            session_service,
        }
    }

    pub async fn logout(
        &self,
        auth_session: AuthSession,
        terminate_all: bool,
        redirect_url: Option<&Url>,
        error_url: Option<&Url>,
    ) -> AuthPage {
        if let Some((user_id, session_key)) = auth_session.user_session().map(|u| (u.user_id, u.key)) {
            match terminate_all {
                true => {
                    log::debug!("Removing all the (non-api-key) tokens for user {user_id}");
                    //remove all non-api-key tokens
                    if let Err(err) = self
                        .token_service
                        .delete_all_by_user(user_id, &[TokenKind::Access, TokenKind::SingleAccess])
                        .await
                    {
                        return self.page_handler.error(auth_session, err, error_url);
                    }

                    log::debug!("Removing all the session for user {user_id}");
                    if let Err(err) = self.session_service.remove_all(user_id).await {
                        log::warn!("Failed to clear all sessions for user {user_id}: {err:?}");
                    }
                }
                false => {
                    log::debug!("Removing remember me token for user, if cookie is present {user_id}");
                    if let Some(token) = auth_session.access().map(|t| t.key.clone()) {
                        log::debug!("Removing token {token} for user {user_id}");
                        if let Err(err) = self.token_service.delete(TokenKind::Access, &token).await {
                            return self.page_handler.error(auth_session, err, error_url);
                        }
                    }

                    log::debug!("Removing session for user {user_id}");
                    if let Err(err) = self.session_service.remove(user_id, &session_key).await {
                        log::warn!("Failed to clear session for user {user_id}: {err:?}");
                    }
                }
            };
        }

        let response_session = auth_session.cleared();
        self.page_handler.redirect(response_session, redirect_url, None)
    }
}

impl AppState {
    pub fn logout_handler(&self) -> LogoutHandler<'_, PgIdentityDb, RedisSessionDb> {
        LogoutHandler::new(self.auth_page_handler(), self.token_service(), self.session_service())
    }
}
