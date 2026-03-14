use crate::{
    app_state::AppState,
    handlers::{AuthPage, AuthPageHandler, UserSessionHandler},
    models::{IdentityError, TokenKind},
    repositories::{
        identity::{pg::PgIdentityDb, IdentityDb},
        session::{redis::RedisSessionDb, SessionDb},
    },
    routes::auth::{AuthSession, TokenCookie},
    services::{SettingsService, TokenService, UserService},
};
use shine_infra::web::extracts::{ClientFingerprint, SiteInfo};
use url::Url;

/// Handler for guest user registration
///
/// Orchestrates new guest user creation, access token generation, and session setup.
pub struct GuestLoginHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    page_handler: AuthPageHandler<'a>,
    user_session_handler: UserSessionHandler<'a, IDB, SDB>,
    settings: &'a SettingsService,
    token_service: &'a TokenService<IDB>,
    user_service: &'a UserService<IDB>,
}

impl<'a, IDB, SDB> GuestLoginHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    pub fn new(
        page_handler: AuthPageHandler<'a>,
        user_session_handler: UserSessionHandler<'a, IDB, SDB>,
        settings: &'a SettingsService,
        token_service: &'a TokenService<IDB>,
        user_service: &'a UserService<IDB>,
    ) -> Self {
        Self {
            page_handler,
            user_session_handler,
            settings,
            token_service,
            user_service,
        }
    }

    /// Register a new guest user and establish a session.
    ///
    /// Creates the user, mints an access token, and creates a session.
    /// The auth_session must already have its prior state cleared by the caller.
    pub async fn register_guest(
        &self,
        auth_session: AuthSession,
        fingerprint: ClientFingerprint,
        site_info: &SiteInfo,
        redirect_url: Option<&Url>,
        error_url: Option<&Url>,
    ) -> AuthPage {
        let identity = match self.user_service.create_with_retry(None, None).await {
            Ok(identity) => identity,
            Err(err) => return self.page_handler.error(auth_session, err, error_url),
        };
        log::debug!("New guest user created: {identity:#?}");

        let user_access = match self
            .token_service
            .create_with_retry(
                identity.id,
                TokenKind::Access,
                &self.settings.token.ttl_access_token,
                Some(&fingerprint),
                None,
                site_info,
            )
            .await
        {
            Ok((token, token_info)) => TokenCookie {
                user_id: token_info.user_id,
                key: token,
                expire_at: token_info.expire_at,
                revoked_token: None,
            },
            Err(err) => return self.page_handler.error(auth_session, err, error_url),
        };

        let user_session = match self
            .user_session_handler
            .create_user_session(&identity, &fingerprint, site_info)
            .await
        {
            Ok(Some(session)) => session,
            Ok(None) => {
                log::warn!("User {} has been deleted during guest login", identity.id);
                return self
                    .page_handler
                    .error(auth_session.with_access(None), IdentityError::UserDeleted, error_url);
            }
            Err(err) => return self.page_handler.error(auth_session, err, error_url),
        };

        log::info!("Guest user registration completed for: {}", identity.id);
        let response_session = auth_session
            .with_access(Some(user_access))
            .with_session(Some(user_session));
        self.page_handler.redirect(response_session, redirect_url, None)
    }
}

impl AppState {
    pub fn guest_login_handler(&self) -> GuestLoginHandler<'_, PgIdentityDb, RedisSessionDb> {
        GuestLoginHandler::new(
            self.auth_page_handler(),
            self.user_session_handler(),
            self.settings(),
            self.token_service(),
            self.user_service(),
        )
    }
}
