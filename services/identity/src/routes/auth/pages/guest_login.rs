use crate::{
    app_state::AppState,
    routes::auth::{AuthPage, AuthPageRequest, AuthSession},
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

    // 5. Clear auth state and register guest
    log::debug!("New user registration flow triggered...");
    let auth_session = req.clear_auth_state().await.into_auth_session();

    state
        .guest_login_handler()
        .register_guest(
            auth_session,
            fingerprint,
            &site_info,
            query.redirect_url.as_ref(),
            query.error_url.as_ref(),
        )
        .await
}
