use crate::{
    app_state::AppState,
    controllers::auth::{AuthError, AuthPage, AuthSession, PageUtils, TokenCookie},
    repositories::identity::{Identity, IdentityError, TokenInfo, TokenKind},
    services::hash_email,
};
use axum::extract::State;
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    typed_header::{TypedHeaderRejection, TypedHeaderRejectionReason},
    TypedHeader,
};
use serde::Deserialize;
use shine_infra::web::{
    extracts::{ClientFingerprint, InputError, SiteInfo, ValidatedQuery},
    responses::ErrorResponse,
};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    remember_me: Option<bool>,
    token: Option<String>,
    redirect_url: Option<Url>,
    error_url: Option<Url>,
    captcha: Option<String>,
}

struct AuthenticationSuccess {
    identity: Identity,
    create_access_token: bool,
    auth_session: AuthSession,
    rotated_token: Option<String>,
}

struct AuthenticationFailure {
    error: AuthError,
    auth_session: AuthSession,
}

async fn complete_email_login(
    state: &AppState,
    query: &QueryParams,
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
    let identity_email_hash = identity.email.map(|email| hash_email(&email));
    // during email verification the captcha is used to check the link is from the email
    let query_email_hash = query.captcha.as_ref();

    if confirmed_email_hash != identity_email_hash
        || confirmed_email_hash.as_ref() != query_email_hash
    {
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
    let identity = match state
        .identity_service()
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
                error: err.into(),
                auth_session,
            })
        }
    };

    Ok(AuthenticationSuccess {
        identity,
        create_access_token: query.remember_me.unwrap_or(false),
        auth_session,
        rotated_token: None,
    })
}

async fn authenticate_with_query_token(
    state: &AppState,
    query: &QueryParams,
    fingerprint: &ClientFingerprint,
    auth_session: AuthSession,
) -> Result<AuthenticationSuccess, AuthenticationFailure> {
    log::debug!("Checking the token from the query ...");
    let (identity, token_info) = {
        let token = query
            .token
            .as_ref()
            .expect("It shall be called only if there is a token in the query");

        // Any token provided as a query token is removed from the DB as it's been used in a non-secure way.
        match state
            .identity_service()
            .take_token(TokenKind::all(), token)
            .await
        {
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
                create_access_token: query.remember_me.unwrap_or(false),
                auth_session: response_session,
                rotated_token: None,
            }),
            TokenKind::EmailAccess => {
                complete_email_login(state, query, token_info, identity, response_session).await
            }
            TokenKind::Persistent => unreachable!(),
            TokenKind::Access => unreachable!(),
        }
    }
}

async fn authenticate_with_header_token(
    state: &AppState,
    query: &QueryParams,
    auth_header: TypedHeader<Authorization<Bearer>>,
    fingerprint: &ClientFingerprint,
    auth_session: AuthSession,
) -> Result<AuthenticationSuccess, AuthenticationFailure> {
    log::debug!("Checking the token from the header...");
    let token = auth_header.token();

    // trying as a single access token
    //  when found revoke even if the login input location is not valid as it is a single use token
    match state
        .identity_service()
        .take_token(TokenKind::all_single_access(), token)
        .await
    {
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
        match state
            .identity_service()
            .test_token(TokenKind::all_multi_access(), token)
            .await
        {
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
        state
            .user_info_handler()
            .revoke_access(token_info.kind, token)
            .await;
        Err(AuthenticationFailure {
            error: AuthError::InvalidToken,
            auth_session: response_session,
        })
    } else if token_info.is_expired {
        log::debug!("Token expired, removing from DB ...");
        state
            .user_info_handler()
            .revoke_access(token_info.kind, token)
            .await;
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
        state
            .user_info_handler()
            .revoke_access(token_info.kind, token)
            .await;
        Err(AuthenticationFailure {
            error: AuthError::TokenExpired,
            auth_session: response_session,
        })
    } else {
        Ok(AuthenticationSuccess {
            identity,
            create_access_token: query.remember_me.unwrap_or(false),
            auth_session: response_session,
            rotated_token: None,
        })
    }
}

async fn authenticate_with_cookie_token(
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
        match state
            .identity_service()
            .test_token(TokenKind::all(), &token)
            .await
        {
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

    // TODO: On network failure (or incomplete cookie update), we may keep a tokens alive, those should be deleted eventually
    // but with this flow we don't know which is the active. Maybe keeping track of "parent" tokens we could delete outdated tokens ...

    log::debug!("Access token flow triggered...");
    let response_session = auth_session
        .with_external_login(None)
        // only clear, but don't revoke the access token, it will be revoked after the new token is acknowledged
        .with_access(None)
        .revoke_session(state)
        .await;

    if let Some(revoked_token) = revoked_token {
        log::debug!("Rotating out the access token ...");
        state
            .user_info_handler()
            .revoke_access(token_info.kind, &revoked_token)
            .await;
    }

    if token_info.kind != TokenKind::Access {
        // it is the sign of a compromised or bogus client
        log::warn!(
            "Non-access token ({:?}) used in the cookie, revoking compromised token ...",
            token_info.kind
        );
        state
            .user_info_handler()
            .revoke_access(token_info.kind, &token)
            .await;
        Err(AuthenticationFailure {
            error: AuthError::InvalidToken,
            auth_session: response_session,
        })
    } else if token_info.is_expired {
        log::debug!("Token expired, removing from DB ...");
        state
            .user_info_handler()
            .revoke_access(token_info.kind, &token)
            .await;
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
        state
            .user_info_handler()
            .revoke_access(token_info.kind, &token)
            .await;
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
        state
            .user_info_handler()
            .revoke_access(token_info.kind, &token)
            .await;
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

async fn authenticate_with_refresh_session(
    state: &AppState,
    query: &QueryParams,
    auth_session: AuthSession,
) -> Result<AuthenticationSuccess, AuthenticationFailure> {
    log::debug!("Checking the session cookie ...");
    assert!(auth_session.access().is_none());
    assert!(auth_session.user_session().is_some());

    let user_id = auth_session.user_session().as_ref().unwrap().user_id;
    let identity = match state.identity_service().find_by_id(user_id).await {
        Ok(Some(info)) => info,
        Ok(None) => {
            return Err(AuthenticationFailure {
                error: IdentityError::UserDeleted.into(),
                auth_session,
            });
        }
        Err(err) => {
            return Err(AuthenticationFailure {
                error: err.into(),
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
        create_access_token: query.remember_me.unwrap_or(false),
        auth_session: response_session,
        rotated_token: None,
    })
}

/// Login flow in priority:
/// - Check token in the query
///   - Headers, cookies and captcha are ignored
///   - Only tokens with single use (SingleAccess, EmailVerify, EmailChange) are allowed
///   - Any other token are rejected and revoked as query parameters are not secure and can be easily copied.
/// - Check authorization header
///   - Query is empty, cookies and captcha are ignored
///   - Only the Persistent token are allowed
///   - Any single access tokens are rejected and revoked
///   - Access token are rejected and revoked as they are exposed only as cookies thus it is a sign of a security issue.
/// - Check the token cookie
///   - Query and headers are empty, captcha is used for email access
///   - Only the Access token is allowed
///   - Any other token are rejected and revoked as cookie should store only Access token, thus it is a sign of a security issue.
async fn authenticate(
    state: &AppState,
    query: &QueryParams,
    auth_header: Result<TypedHeader<Authorization<Bearer>>, TypedHeaderRejection>,
    auth_session: AuthSession,
    fingerprint: &ClientFingerprint,
) -> Result<AuthenticationSuccess, AuthenticationFailure> {
    if query.token.is_some() {
        return authenticate_with_query_token(state, query, fingerprint, auth_session).await;
    }

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
        return authenticate_with_header_token(
            state,
            query,
            auth_header,
            fingerprint,
            auth_session,
        )
        .await;
    }

    if auth_session.access().is_some() {
        return authenticate_with_cookie_token(state, fingerprint, auth_session).await;
    }

    if auth_session.user_session().is_some() {
        return authenticate_with_refresh_session(state, query, auth_session).await;
    }

    Err(AuthenticationFailure {
        error: AuthError::LoginRequired,
        auth_session,
    })
}

/// Login with token using query, auth and cookie as sources.
#[utoipa::path(
    get,
    path = "/auth/token/login",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Login with a token")
    )
)]
pub async fn token_login(
    State(state): State<AppState>,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
    auth_header: Result<TypedHeader<Authorization<Bearer>>, TypedHeaderRejection>,
    auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, error.problem, None, None),
    };

    log::debug!("Query: {:#?}", query);

    // clear external login cookie, it shall be only for the authorize callback from the external provider
    let auth_session = auth_session.with_external_login(None);

    let AuthenticationSuccess {
        identity,
        create_access_token,
        auth_session,
        rotated_token,
    } = match authenticate(&state, &query, auth_header, auth_session, &fingerprint).await {
        Ok(success) => success,
        Err(failure) => {
            return PageUtils::new(&state).error(
                failure.auth_session,
                failure.error,
                query.error_url.as_ref(),
                query.redirect_url.as_ref(),
            );
        }
    };

    assert!(
        auth_session.user_session().is_none(),
        "Session shall have been cleared"
    );
    assert!(
        auth_session.external_login().is_none(),
        "External login cookie shall have been cleared"
    );

    // Create a new access token. It can be either a rotated or a new token
    let auth_session = if create_access_token {
        log::debug!("Creating access token for identity: {:#?}", identity);
        // create a new access token
        let user_token = match state
            .login_token_handler()
            .create_user_token(
                identity.id,
                TokenKind::Access,
                &state.settings().token.ttl_access_token,
                Some(&fingerprint),
                &site_info,
            )
            .await
        {
            Ok(user_token) => user_token,
            Err(err) => {
                return PageUtils::new(&state).error(
                    auth_session,
                    err,
                    query.error_url.as_ref(),
                    query.redirect_url.as_ref(),
                )
            }
        };

        // preserve the old token in case client does not acknowledge the new one
        auth_session
            .with_access(Some(TokenCookie {
                user_id: user_token.user_id,
                key: user_token.token,
                expire_at: user_token.expire_at,
                revoked_token: rotated_token,
            }))
            .with_session(None)
    } else {
        auth_session.with_access(None).with_session(None)
    };

    // Create a new user session.
    let auth_session = {
        let user_session = match state
            .user_info_handler()
            .create_user_session(&identity, &fingerprint, &site_info)
            .await
        {
            Ok(Some(session)) => session,
            Ok(None) => {
                log::warn!("User {} has been deleted during login", identity.id);
                return PageUtils::new(&state).error(
                    auth_session.with_access(None),
                    IdentityError::UserDeleted,
                    query.error_url.as_ref(),
                    query.redirect_url.as_ref(),
                );
            }
            Err(err) => {
                return PageUtils::new(&state).error(
                    auth_session,
                    err,
                    query.error_url.as_ref(),
                    query.redirect_url.as_ref(),
                )
            }
        };
        auth_session.with_session(Some(user_session))
    };

    log::info!("Token login completed for: {}", identity.id);
    PageUtils::new(&state).redirect(auth_session, query.redirect_url.as_ref(), None)
}
