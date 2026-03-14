use crate::{
    app_state::AppState,
    handlers::{AuthPage, AuthPageHandler},
    repositories::{
        identity::{pg::PgIdentityDb, IdentityDb},
        session::{redis::RedisSessionDb, SessionDb},
    },
    routes::auth::{AuthError, AuthSession},
    services::{SessionService, UserService},
};
use url::Url;

/// Handler for user account deletion
///
/// Orchestrates session validation, user deletion, and session cleanup.
pub struct DeleteUserHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    page_handler: AuthPageHandler<'a>,
    user_service: &'a UserService<IDB>,
    session_service: &'a SessionService<SDB>,
}

impl<'a, IDB, SDB> DeleteUserHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    pub fn new(
        page_handler: AuthPageHandler<'a>,
        user_service: &'a UserService<IDB>,
        session_service: &'a SessionService<SDB>,
    ) -> Self {
        Self {
            page_handler,
            user_service,
            session_service,
        }
    }

    pub async fn delete_user(
        &self,
        auth_session: AuthSession,
        confirmation: Option<&str>,
        redirect_url: Option<&Url>,
        error_url: Option<&Url>,
    ) -> AuthPage {
        let (user_id, user_name, session_key) =
            match auth_session.user_session().map(|u| (u.user_id, u.name.clone(), u.key)) {
                Some(user) => user,
                None => {
                    return self
                        .page_handler
                        .error(auth_session, AuthError::LoginRequired, error_url)
                }
            };

        // check for user confirmation
        if confirmation != Some(user_name.as_str()) {
            return self
                .page_handler
                .error(auth_session, AuthError::MissingConfirmation, error_url);
        }

        // validate session as this is a very risky operation
        match self.session_service.find(user_id, &session_key).await {
            Ok(None) => {
                return self
                    .page_handler
                    .error(auth_session, AuthError::SessionExpired, error_url)
            }
            Err(err) => return self.page_handler.error(auth_session, err, error_url),
            Ok(Some(_)) => {}
        };

        if let Err(err) = self.user_service.delete(user_id).await {
            return self.page_handler.error(auth_session, err, error_url);
        }

        // End of validations, from this point
        //  - there is no reason to keep session
        //  - errors are irrelevant for the users and mostly just warnings.
        let response_session = auth_session.cleared();

        if let Err(err) = self.session_service.remove_all(user_id).await {
            log::warn!("Failed to clear all sessions for user {user_id}: {err:?}");
        }

        self.page_handler.redirect(response_session, redirect_url, None)
    }
}

impl AppState {
    pub fn delete_user_handler(&self) -> DeleteUserHandler<'_, PgIdentityDb, RedisSessionDb> {
        DeleteUserHandler::new(self.auth_page_handler(), self.user_service(), self.session_service())
    }
}
