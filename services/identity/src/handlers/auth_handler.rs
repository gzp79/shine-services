use crate::{
    app_state::AppState,
    models::{Identity, IdentityError, TokenInfo, TokenKind},
    repositories::identity::{pg::PgIdentityDb, IdentityDb},
    routes::auth::{AuthError, AuthSession},
    services::{TokenService, UserService},
};
use axum_extra::headers::{authorization::Bearer, Authorization};
use axum_extra::typed_header::TypedHeader;
use shine_infra::{models::hash_email, web::extracts::ClientFingerprint};

/// Result of a successful authentication attempt
pub struct AuthenticationSuccess {
    pub identity: Identity,
    pub create_access_token: bool,
    pub auth_session: AuthSession,
    pub rotated_token: Option<String>,
}

/// Result of a failed authentication attempt
pub struct AuthenticationFailure {
    pub error: AuthError,
    pub auth_session: AuthSession,
}

/// Handler for authentication operations
///
/// Orchestrates authentication logic across user_service, token_service, and session management.
/// Extracted from token_login.rs to make authentication logic reusable across routes.
pub struct AuthHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    token_service: &'a TokenService<IDB>,
    user_service: &'a UserService<IDB>,
}

impl<'a, IDB> AuthHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    pub fn new(token_service: &'a TokenService<IDB>, user_service: &'a UserService<IDB>) -> Self {
        AuthHandler { token_service, user_service }
    }

    /// Complete email verification login flow
    ///
    /// Validates that the email in the token matches the user's email and the captcha,
    /// then updates the user's email verification status.
    pub async fn authenticate_with_email_token(
        &self,
        remember_me: bool,
        captcha: Option<&str>,
        token_info: TokenInfo,
        identity: Identity,
        auth_session: AuthSession,
    ) -> Result<AuthenticationSuccess, AuthenticationFailure> {
        log::debug!("Completing email verification...");

        let confirmed_email = token_info
            .bound_email
            .as_deref()
            .expect("Email shall be bound to the token");

        let confirmed_email_hash = Some(hash_email(confirmed_email));
        let identity_email_hash = identity.email.as_ref().map(|email| email.hash());
        // during email verification the captcha is used to check the link is from the email
        let query_email_hash = captcha.map(|s| s.to_string());

        if confirmed_email_hash != identity_email_hash || confirmed_email_hash.as_ref() != query_email_hash.as_ref() {
            log::info!(
                "Identity {} has non-matching emails to verify. [{:?}], [{:?}], [{:?}]",
                identity.id,
                confirmed_email_hash,
                identity_email_hash,
                query_email_hash
            );
            return Err(AuthenticationFailure {
                error: AuthError::EmailConflict,
                auth_session,
            });
        }

        log::info!("Updating email verification for identity {}.", identity.id);
        let identity = match self
            .user_service
            .update(identity.id, None, Some((confirmed_email, true)))
            .await
        {
            Ok(Some(identity)) => identity,
            Ok(None) => {
                return Err(AuthenticationFailure {
                    error: IdentityError::UserDeleted.into(),
                    auth_session,
                })
            }
            Err(err) => {
                return Err(AuthenticationFailure {
                    error: AuthError::from(err),
                    auth_session,
                })
            }
        };

        Ok(AuthenticationSuccess {
            identity,
            create_access_token: remember_me,
            auth_session,
            rotated_token: None,
        })
    }

    /// Authenticate using a token from the query string
    ///
    /// Query tokens are single-use and are consumed immediately.
    /// Handles both SingleAccess and EmailAccess token types.
    pub async fn authenticate_with_query_token(
        &self,
        state: &AppState,
        token: &str,
        remember_me: bool,
        captcha: Option<&str>,
        fingerprint: &ClientFingerprint,
        auth_session: AuthSession,
    ) -> Result<AuthenticationSuccess, AuthenticationFailure> {
        log::debug!("Checking the token from the query ...");
        let (identity, token_info) = {
            // Any token provided as a query token is removed from the DB as it's been used in a non-secure way.
            match self.token_service.take(TokenKind::all(), token).await {
                Ok(Some(info)) => info,
                Ok(None) => {
                    log::debug!("Expired single access token ...");
                    return Err(AuthenticationFailure {
                        error: AuthError::TokenExpired,
                        auth_session: auth_session
                            .with_external_login(None)
                            .revoke_access(state)
                            .await
                            .revoke_session(state)
                            .await,
                    });
                }
                Err(err) => {
                    return Err(AuthenticationFailure {
                        error: err.into(),
                        auth_session,
                    })
                }
            }
        };

        log::debug!("Single access token flow triggered...");
        let response_session = auth_session
            .with_external_login(None)
            .revoke_access(state)
            .await
            .revoke_session(state)
            .await;

        if !token_info.kind.is_single_access() {
            // it is the sign of a compromised or bogus client
            log::warn!("Non-single access token used in the query, revoking compromised token ...");
            Err(AuthenticationFailure {
                error: AuthError::InvalidToken,
                auth_session: response_session,
            })
        } else if token_info.is_expired {
            Err(AuthenticationFailure {
                error: AuthError::TokenExpired,
                auth_session: response_session,
            })
        } else if !token_info.check_fingerprint(fingerprint) {
            log::info!(
                "Client fingerprint changed [{:?}] -> [{:#?}]",
                token_info.bound_fingerprint,
                fingerprint
            );
            Err(AuthenticationFailure {
                error: AuthError::TokenExpired,
                auth_session: response_session,
            })
        } else {
            match token_info.kind {
                TokenKind::SingleAccess => Ok(AuthenticationSuccess {
                    identity,
                    create_access_token: remember_me,
                    auth_session: response_session,
                    rotated_token: None,
                }),
                TokenKind::EmailAccess => {
                    self.authenticate_with_email_token(remember_me, captcha, token_info, identity, response_session)
                        .await
                }
                TokenKind::Persistent => unreachable!(),
                TokenKind::Access => unreachable!(),
            }
        }
    }

    /// Authenticate using a bearer token from the Authorization header
    ///
    /// Header tokens are multi-use Persistent tokens.
    /// Single-access tokens in headers are rejected.
    pub async fn authenticate_with_header_token(
        &self,
        state: &AppState,
        auth_header: TypedHeader<Authorization<Bearer>>,
        remember_me: bool,
        fingerprint: &ClientFingerprint,
        auth_session: AuthSession,
    ) -> Result<AuthenticationSuccess, AuthenticationFailure> {
        log::debug!("Checking the token from the header...");
        let token = auth_header.token();

        // trying as a single access token
        //  when found revoke even if the login input location is not valid as it is a single use token
        match self.token_service.take(TokenKind::all_single_access(), token).await {
            Ok(Some(_)) => {
                log::debug!("Single access token used in the Persistent token flow, revoking it ...");
                return Err(AuthenticationFailure {
                    error: AuthError::InvalidToken,
                    auth_session,
                });
            }
            Ok(None) => {}
            Err(err) => {
                return Err(AuthenticationFailure {
                    error: err.into(),
                    auth_session,
                });
            }
        }

        // now trying it as a multi-use token
        let (identity, token_info) = {
            match self.token_service.test(TokenKind::all_multi_access(), token).await {
                Ok(Some(info)) => info,
                Ok(None) => {
                    log::debug!("Invalid or expired Persistent token ...");
                    return Err(AuthenticationFailure {
                        error: AuthError::TokenExpired,
                        auth_session: auth_session.with_access(None),
                    });
                }
                Err(err) => {
                    return Err(AuthenticationFailure {
                        error: err.into(),
                        auth_session,
                    });
                }
            }
        };

        log::debug!("Persistent token flow triggered...");
        let response_session = auth_session
            // the client will get a new access,
            .revoke_access(state)
            .await
            // the client will get a new session
            .revoke_session(state)
            .await;

        if token_info.kind != TokenKind::Persistent {
            // it is the sign of a compromised or bogus client
            log::warn!(
                "Non-persistent token ({:?}) used in the header, revoking compromised token ...",
                token_info.kind
            );
            if let Err(err) = self.token_service.delete(token_info.kind, token).await {
                log::error!("Failed to revoke token: {err}");
            }
            Err(AuthenticationFailure {
                error: AuthError::InvalidToken,
                auth_session: response_session,
            })
        } else if token_info.is_expired {
            log::debug!("Token expired, removing from DB ...");
            if let Err(err) = self.token_service.delete(token_info.kind, token).await {
                log::error!("Failed to revoke token: {err}");
            }
            Err(AuthenticationFailure {
                error: AuthError::TokenExpired,
                auth_session: response_session,
            })
        } else if !token_info.check_fingerprint(fingerprint) {
            log::info!(
                "Client fingerprint changed [{:?}] -> [{:#?}]",
                token_info.bound_fingerprint,
                fingerprint
            );
            if let Err(err) = self.token_service.delete(token_info.kind, token).await {
                log::error!("Failed to revoke token: {err}");
            }
            Err(AuthenticationFailure {
                error: AuthError::TokenExpired,
                auth_session: response_session,
            })
        } else {
            Ok(AuthenticationSuccess {
                identity,
                create_access_token: remember_me,
                auth_session: response_session,
                rotated_token: None,
            })
        }
    }

    /// Authenticate using an access token from cookies
    ///
    /// Cookie tokens are Access tokens that get rotated for security.
    /// Implements token rotation to prevent replay attacks.
    pub async fn authenticate_with_cookie_token(
        &self,
        state: &AppState,
        fingerprint: &ClientFingerprint,
        auth_session: AuthSession,
    ) -> Result<AuthenticationSuccess, AuthenticationFailure> {
        log::debug!("Checking the token from the cookie ...");

        let (token_user_id, token, revoked_token) = auth_session
            .access()
            .as_ref()
            .map(|t| (t.user_id, t.key.clone(), t.revoked_token.clone()))
            .expect("It shall be called only if there is a token cookie");

        // we skip the single access token. It is a bit less secure, but
        // cookies are sent securely and any non-access token wil be revoked later in the flow

        let (identity, token_info) = {
            match self.token_service.test(TokenKind::all(), &token).await {
                Ok(Some(info)) => info,
                Ok(None) => {
                    log::debug!("Invalid or expired Access token ...");
                    return Err(AuthenticationFailure {
                        error: AuthError::TokenExpired,
                        auth_session: auth_session.with_access(None),
                    });
                }
                Err(err) => {
                    return Err(AuthenticationFailure {
                        error: err.into(),
                        auth_session,
                    })
                }
            }
        };

        // Token rotation:
        // - the "old/active" token (from the cookie), that is used for login and should be revoked
        // - the "revoked" token (from the cookie), that was rotated out in a previous login
        // - the "new" token (just generated), that is used for the next login
        // When a cookie with the "old" token is received containing a "revoked" token, it indicates the rotation was successful as
        // client has just used rotated token. To make sure a network issue won't lock out users with only access tokens,
        // "old/active" token is kept alive for another round while client can confirm a successful rotation.

        log::debug!("Access token flow triggered...");
        let response_session = auth_session
            .with_external_login(None)
            // only clear, but don't revoke the access token, it will be revoked after the new token is acknowledged
            .with_access(None)
            .revoke_session(state)
            .await;

        if let Some(revoked_token) = revoked_token {
            log::debug!("Rotating out the access token ...");
            if let Err(err) = self.token_service.delete(token_info.kind, &revoked_token).await {
                log::error!("Failed to revoke rotated token: {err}");
            }
        }

        if token_info.kind != TokenKind::Access {
            // it is the sign of a compromised or bogus client
            log::warn!(
                "Non-access token ({:?}) used in the cookie, revoking compromised token ...",
                token_info.kind
            );
            if let Err(err) = self.token_service.delete(token_info.kind, &token).await {
                log::error!("Failed to revoke token: {err}");
            }
            Err(AuthenticationFailure {
                error: AuthError::InvalidToken,
                auth_session: response_session,
            })
        } else if token_info.is_expired {
            log::debug!("Token expired, removing from DB ...");
            if let Err(err) = self.token_service.delete(token_info.kind, &token).await {
                log::error!("Failed to revoke token: {err}");
            }
            Err(AuthenticationFailure {
                error: AuthError::TokenExpired,
                auth_session: response_session,
            })
        } else if identity.id != token_user_id {
            // it is the sign of a compromised or bogus client
            log::warn!(
                "User is not matching in identity ({}) and in cookie ({}), token: [{}]",
                identity.id,
                token_user_id,
                token
            );
            if let Err(err) = self.token_service.delete(token_info.kind, &token).await {
                log::error!("Failed to revoke token: {err}");
            }
            Err(AuthenticationFailure {
                error: AuthError::InvalidToken,
                auth_session: response_session,
            })
        } else if !token_info.check_fingerprint(fingerprint) {
            log::info!(
                "Client fingerprint changed [{:?}] -> [{:#?}]",
                token_info.bound_fingerprint,
                fingerprint
            );
            if let Err(err) = self.token_service.delete(token_info.kind, &token).await {
                log::error!("Failed to revoke token: {err}");
            }
            Err(AuthenticationFailure {
                error: AuthError::TokenExpired,
                auth_session: response_session,
            })
        } else {
            Ok(AuthenticationSuccess {
                identity,
                create_access_token: true,
                auth_session: response_session,
                rotated_token: Some(token),
            })
        }
    }

    /// Authenticate using an active session (refresh flow)
    ///
    /// Used when the user has a valid session but no access token.
    /// This is the session refresh flow.
    pub async fn authenticate_with_refresh_session(
        &self,
        state: &AppState,
        remember_me: bool,
        auth_session: AuthSession,
    ) -> Result<AuthenticationSuccess, AuthenticationFailure> {
        log::debug!("Checking the session cookie ...");
        assert!(auth_session.access().is_none());
        assert!(auth_session.user_session().is_some());

        let user_id = auth_session.user_session().as_ref().unwrap().user_id;
        let identity = match self.user_service.find_by_id(user_id).await {
            Ok(Some(info)) => info,
            Ok(None) => {
                return Err(AuthenticationFailure {
                    error: IdentityError::UserDeleted.into(),
                    auth_session,
                });
            }
            Err(err) => {
                return Err(AuthenticationFailure {
                    error: AuthError::from(err),
                    auth_session,
                })
            }
        };

        log::debug!("Refresh session flow triggered...");
        let response_session = auth_session
            .with_external_login(None)
            // only clear, but don't revoke the access token, it will be revoked after the new token is acknowledged
            .revoke_access(state)
            .await
            .revoke_session(state)
            .await;

        Ok(AuthenticationSuccess {
            identity,
            create_access_token: remember_me,
            auth_session: response_session,
            rotated_token: None,
        })
    }

    /// Authenticate user using multiple methods
    ///
    /// Tries authentication methods in priority order:
    /// 1. Query token (single-use)
    /// 2. Authorization header (persistent token)
    /// 3. Cookie token (access token with rotation)
    /// 4. Session refresh (existing session, no token)
    pub async fn authenticate_user(
        &self,
        state: &AppState,
        token: Option<&str>,
        remember_me: bool,
        captcha: Option<&str>,
        auth_header: Result<
            axum_extra::TypedHeader<axum_extra::headers::Authorization<axum_extra::headers::authorization::Bearer>>,
            axum_extra::typed_header::TypedHeaderRejection,
        >,
        auth_session: AuthSession,
        fingerprint: &ClientFingerprint,
    ) -> Result<AuthenticationSuccess, AuthenticationFailure> {
        use crate::routes::auth::AuthError;
        use axum_extra::typed_header::TypedHeaderRejectionReason;

        // Priority 1: Query token
        if let Some(token) = token {
            return self
                .authenticate_with_query_token(state, token, remember_me, captcha, fingerprint, auth_session)
                .await;
        }

        // Priority 2: Authorization header
        let auth_header = match auth_header {
            Ok(auth_header) => Some(auth_header),
            Err(err) if matches!(err.reason(), TypedHeaderRejectionReason::Missing) => None,
            Err(_) => {
                return Err(AuthenticationFailure {
                    error: AuthError::InvalidHeader,
                    auth_session,
                });
            }
        };
        if let Some(auth_header) = auth_header {
            return self
                .authenticate_with_header_token(state, auth_header, remember_me, fingerprint, auth_session)
                .await;
        }

        // Priority 3: Cookie token
        if auth_session.access().is_some() {
            return self
                .authenticate_with_cookie_token(state, fingerprint, auth_session)
                .await;
        }

        // Priority 4: Session refresh
        if auth_session.user_session().is_some() {
            return self
                .authenticate_with_refresh_session(state, remember_me, auth_session)
                .await;
        }

        // No authentication method available
        Err(AuthenticationFailure {
            error: AuthError::LoginRequired,
            auth_session,
        })
    }
}

impl AppState {
    pub fn auth_handler(&self) -> AuthHandler<'_, PgIdentityDb> {
        AuthHandler::new(self.token_service(), self.user_service())
    }
}
