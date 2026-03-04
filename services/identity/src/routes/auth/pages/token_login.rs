use crate::{
    app_state::AppState,
    handlers::{AuthHandler, AuthenticationFailure, AuthenticationSuccess},
    repositories::identity::{IdentityError, TokenKind},
    routes::auth::{AuthError, AuthPage, AuthPageRequest, AuthSession, PageUtils, TokenCookie},
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
    let handler = AuthHandler::new(state);

    if let Some(ref token) = query.token {
        return handler
            .authenticate_with_query_token(
                state,
                token,
                query.remember_me.unwrap_or(false),
                query.captcha.as_deref(),
                fingerprint,
                auth_session,
            )
            .await;
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
        return handler
            .authenticate_with_header_token(
                state,
                auth_header,
                query.remember_me.unwrap_or(false),
                fingerprint,
                auth_session,
            )
            .await;
    }

    if auth_session.access().is_some() {
        return handler
            .authenticate_with_cookie_token(state, fingerprint, auth_session)
            .await;
    }

    if auth_session.user_session().is_some() {
        return handler
            .authenticate_with_refresh_session(state, query.remember_me.unwrap_or(false), auth_session)
            .await;
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
    // 1. Create request helper
    let req = AuthPageRequest::new(&state, auth_session);

    // 2. Validate query
    let query = match req.validate_query(query) {
        Ok(q) => q,
        Err(page) => return page,
    };

    // 3. Validate redirect URLs
    if let Some(page) = req.validate_redirect_urls(query.redirect_url.as_ref(), query.error_url.as_ref()) {
        return page;
    }

    log::debug!("Query: {query:#?}");

    // 4. Clear external login cookie (shall be only for authorize callback from external provider)
    let auth_session = req.into_auth_session().with_external_login(None);

    let AuthenticationSuccess {
        identity,
        create_access_token,
        auth_session,
        rotated_token,
    } = match authenticate(&state, &query, auth_header, auth_session, &fingerprint).await {
        Ok(success) => success,
        Err(failure) => {
            return PageUtils::new(&state).error(failure.auth_session, failure.error, query.error_url.as_ref());
        }
    };

    assert!(auth_session.user_session().is_none(), "Session shall have been cleared");
    assert!(
        auth_session.external_login().is_none(),
        "External login cookie shall have been cleared"
    );

    // Create a new access token. It can be either a rotated or a new token
    let auth_session = if create_access_token {
        log::debug!("Creating access token for identity: {identity:#?}");
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
            Err(err) => return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref()),
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
        let user_session = match state.create_user_session(&identity, &fingerprint, &site_info).await {
            Ok(Some(session)) => session,
            Ok(None) => {
                log::warn!("User {} has been deleted during login", identity.id);
                return PageUtils::new(&state).error(
                    auth_session.with_access(None),
                    IdentityError::UserDeleted,
                    query.error_url.as_ref(),
                );
            }
            Err(err) => return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref()),
        };
        auth_session.with_session(Some(user_session))
    };

    log::info!("Token login completed for: {}", identity.id);
    PageUtils::new(&state).redirect(auth_session, query.redirect_url.as_ref(), None)
}
