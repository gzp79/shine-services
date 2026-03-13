use crate::{
    app_state::AppState,
    handlers::{AuthHandler, AuthenticationSuccess},
    models::{IdentityError, TokenKind},
    routes::auth::{AuthPage, AuthPageRequest, AuthSession, TokenCookie},
};
use axum::extract::State;
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    typed_header::TypedHeaderRejection,
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
    } = match AuthHandler::new(&state)
        .authenticate_user(
            &state,
            query.token.as_deref(),
            query.remember_me.unwrap_or(false),
            query.captcha.as_deref(),
            auth_header,
            auth_session,
            &fingerprint,
        )
        .await
    {
        Ok(success) => success,
        Err(failure) => {
            return state
                .auth_page_handler()
                .error(failure.auth_session, failure.error, query.error_url.as_ref());
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
        let (token, token_info) = match state
            .token_service()
            .create_with_retry(
                identity.id,
                TokenKind::Access,
                &state.settings().token.ttl_access_token,
                Some(&fingerprint),
                None,
                &site_info,
            )
            .await
        {
            Ok(result) => result,
            Err(err) => {
                return state
                    .auth_page_handler()
                    .error(auth_session, err, query.error_url.as_ref())
            }
        };

        // preserve the old token in case client does not acknowledge the new one
        auth_session
            .with_access(Some(TokenCookie {
                user_id: identity.id,
                key: token,
                expire_at: token_info.expire_at,
                revoked_token: rotated_token,
            }))
            .with_session(None)
    } else {
        auth_session.with_access(None).with_session(None)
    };

    // Create a new user session.
    let auth_session = {
        let user_session = match state
            .user_session_handler()
            .create_user_session(&identity, &fingerprint, &site_info)
            .await
        {
            Ok(Some(session)) => session,
            Ok(None) => {
                log::warn!("User {} has been deleted during login", identity.id);
                return state.auth_page_handler().error(
                    auth_session.with_access(None),
                    IdentityError::UserDeleted,
                    query.error_url.as_ref(),
                );
            }
            Err(err) => {
                return state
                    .auth_page_handler()
                    .error(auth_session, err, query.error_url.as_ref())
            }
        };
        auth_session.with_session(Some(user_session))
    };

    log::info!("Token login completed for: {}", identity.id);
    state
        .auth_page_handler()
        .redirect(auth_session, query.redirect_url.as_ref(), None)
}
