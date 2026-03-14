use super::credential_handler::{CredentialHandler, TokenIssuance};
use crate::{
    app_state::AppState,
    handlers::{AuthPage, AuthPageHandler},
    repositories::{
        identity::{pg::PgIdentityDb, IdentityDb},
        session::{redis::RedisSessionDb, SessionDb},
    },
    routes::auth::AuthSession,
    services::UserService,
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
    credential_handler: CredentialHandler<'a, IDB, SDB>,
    user_service: &'a UserService<IDB>,
}

impl<'a, IDB, SDB> GuestLoginHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    pub fn new(
        page_handler: AuthPageHandler<'a>,
        credential_handler: CredentialHandler<'a, IDB, SDB>,
        user_service: &'a UserService<IDB>,
    ) -> Self {
        Self {
            page_handler,
            credential_handler,
            user_service,
        }
    }

    /// Register a new guest user and establish a session.
    ///
    /// Creates the user then delegates token minting and session setup to
    /// `CredentialHandler`. The auth_session must already have its prior state
    /// cleared by the caller.
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

        self.credential_handler
            .establish(
                identity,
                TokenIssuance::Create,
                auth_session,
                &fingerprint,
                site_info,
                redirect_url,
                error_url,
            )
            .await
    }
}

impl AppState {
    pub fn guest_login_handler(&self) -> GuestLoginHandler<'_, PgIdentityDb, RedisSessionDb> {
        GuestLoginHandler::new(self.auth_page_handler(), self.credential_handler(), self.user_service())
    }
}
