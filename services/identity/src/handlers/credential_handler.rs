use crate::{
    app_state::AppState,
    handlers::{AuthPage, AuthPageHandler, UserSessionHandler},
    models::{Identity, IdentityError, TokenKind},
    repositories::{
        identity::{pg::PgIdentityDb, IdentityDb},
        session::{redis::RedisSessionDb, SessionDb},
    },
    routes::auth::{AuthSession, TokenCookie},
    services::{SettingsService, TokenService},
};
use shine_infra::web::extracts::{ClientFingerprint, SiteInfo};
use url::Url;

/// Describes how to issue the access token when completing a login.
pub enum TokenIssuance {
    /// Do not create an access token.
    Skip,
    /// Create a fresh access token.
    Create,
    /// Create a fresh access token and retire the previous one (cookie rotation).
    Rotate(String),
}

/// Handler for establishing user credentials after a successful authentication.
///
/// Orchestrates access token creation (or rotation) and user session setup.
pub struct CredentialHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    page_handler: AuthPageHandler<'a>,
    user_session_handler: UserSessionHandler<'a, IDB, SDB>,
    settings: &'a SettingsService,
    token_service: &'a TokenService<IDB>,
}

impl<'a, IDB, SDB> CredentialHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    pub fn new(
        page_handler: AuthPageHandler<'a>,
        user_session_handler: UserSessionHandler<'a, IDB, SDB>,
        settings: &'a SettingsService,
        token_service: &'a TokenService<IDB>,
    ) -> Self {
        Self {
            page_handler,
            user_session_handler,
            settings,
            token_service,
        }
    }

    pub async fn establish(
        &self,
        identity: Identity,
        issuance: TokenIssuance,
        auth_session: AuthSession,
        fingerprint: &ClientFingerprint,
        site_info: &SiteInfo,
        redirect_url: Option<&Url>,
        error_url: Option<&Url>,
    ) -> AuthPage {
        assert!(auth_session.user_session().is_none(), "Session shall have been cleared");
        assert!(
            auth_session.external_login().is_none(),
            "External login cookie shall have been cleared"
        );

        let auth_session = match issuance {
            TokenIssuance::Skip => auth_session.with_access(None).with_session(None),
            TokenIssuance::Create | TokenIssuance::Rotate(_) => {
                let rotated_token = if let TokenIssuance::Rotate(t) = issuance {
                    Some(t)
                } else {
                    None
                };
                log::debug!("Creating access token for identity: {identity:#?}");
                let (token, token_info) = match self
                    .token_service
                    .create_with_retry(
                        identity.id,
                        TokenKind::Access,
                        &self.settings.token.ttl_access_token,
                        Some(fingerprint),
                        None,
                        site_info,
                    )
                    .await
                {
                    Ok(result) => result,
                    Err(err) => return self.page_handler.error(auth_session, err, error_url),
                };
                auth_session
                    .with_access(Some(TokenCookie {
                        user_id: identity.id,
                        key: token,
                        expire_at: token_info.expire_at,
                        revoked_token: rotated_token,
                    }))
                    .with_session(None)
            }
        };

        let auth_session = {
            let user_session = match self
                .user_session_handler
                .create_user_session(&identity, fingerprint, site_info)
                .await
            {
                Ok(Some(session)) => session,
                Ok(None) => {
                    log::warn!("User {} has been deleted during login", identity.id);
                    return self.page_handler.error(
                        auth_session.with_access(None),
                        IdentityError::UserDeleted,
                        error_url,
                    );
                }
                Err(err) => return self.page_handler.error(auth_session, err, error_url),
            };
            auth_session.with_session(Some(user_session))
        };

        log::info!("Credentials established for: {}", identity.id);
        self.page_handler.redirect(auth_session, redirect_url, None)
    }
}

impl AppState {
    pub fn credential_handler(&self) -> CredentialHandler<'_, PgIdentityDb, RedisSessionDb> {
        CredentialHandler::new(
            self.auth_page_handler(),
            self.user_session_handler(),
            self.settings(),
            self.token_service(),
        )
    }
}
