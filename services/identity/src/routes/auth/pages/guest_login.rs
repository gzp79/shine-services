use crate::{
    app_state::AppState,
    repositories::identity::{IdentityError, TokenKind},
    routes::auth::{AuthPage, AuthPageRequest, AuthSession, TokenCookie},
};
use axum::extract::State;
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
    redirect_url: Option<Url>,
    error_url: Option<Url>,
    captcha: Option<String>,
}

/// Login with token using query, auth and cookie as sources.
#[utoipa::path(
    get,
    path = "/auth/guest/login",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Register a new guest user")
    )
)]
pub async fn guest_login(
    State(state): State<AppState>,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
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

    // 4. Validate captcha
    if let Some(page) = req
        .validate_captcha(query.captcha.as_deref(), query.error_url.as_ref())
        .await
    {
        return page;
    }

    // 5. Clear auth state
    log::debug!("New user registration flow triggered...");
    let req = req.clear_auth_state().await;

    // 6. Business logic - create a new user
    let identity = match req.state().user_service().create_with_retry(None, None).await {
        Ok(identity) => identity,
        Err(err) => return req.error_page(err, query.error_url.as_ref()),
    };
    log::debug!("New user created: {identity:#?}");

    // Create access token
    let user_access = {
        let user_token = match req
            .state()
            .login_token_handler()
            .create_user_token(
                identity.id,
                TokenKind::Access,
                &req.state().settings().token.ttl_access_token,
                Some(&fingerprint),
                &site_info,
            )
            .await
        {
            Ok(user_token) => user_token,
            Err(err) => return req.error_page(err, query.error_url.as_ref()),
        };

        TokenCookie {
            user_id: user_token.user_id,
            key: user_token.token,
            expire_at: user_token.expire_at,
            revoked_token: None,
        }
    };

    // Create user session.
    let user_session = match req
        .state()
        .create_user_session(&identity, &fingerprint, &site_info)
        .await
    {
        Ok(Some(session)) => session,
        Ok(None) => {
            log::warn!("User {} has been deleted during login", identity.id);
            use crate::routes::auth::PageUtils;
            return PageUtils::new(req.state()).error(
                req.auth_session().clone().with_access(None),
                IdentityError::UserDeleted,
                query.error_url.as_ref(),
            );
        }
        Err(err) => return req.error_page(err, query.error_url.as_ref()),
    };

    // 7. Return response
    log::info!("Guest user registration completed for: {}", identity.id);
    let final_session = req
        .into_auth_session()
        .with_access(Some(user_access))
        .with_session(Some(user_session));
    use crate::routes::auth::PageUtils;
    PageUtils::new(&state).redirect(final_session, query.redirect_url.as_ref(), None)
}
