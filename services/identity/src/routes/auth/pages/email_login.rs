use crate::{
    app_state::AppState,
    routes::auth::{AuthError, AuthPage, AuthPageRequest, AuthSession},
};
use axum::extract::State;
use serde::Deserialize;
use shine_infra::{
    language::Language,
    web::{
        extracts::{InputError, SiteInfo, ValidatedQuery},
        responses::ErrorResponse,
    },
};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    #[validate(email)]
    email: String,
    redirect_url: Option<Url>,
    error_url: Option<Url>,
    remember_me: Option<bool>,
    captcha: Option<String>,
    lang: Option<Language>,
}

/// Login with token using query, auth and cookie as sources.
#[utoipa::path(
    get,
    path = "/auth/email/login",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Start an email login flow")
    )
)]
pub async fn email_login(
    State(state): State<AppState>,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
    auth_session: AuthSession,
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
    log::debug!("Email login flow triggered...");
    let req = req.clear_auth_state().await;

    // 6. Business logic
    let identity = match state
        .login_email_handler()
        .send_login_email(
            &query.email,
            query.remember_me,
            query.redirect_url.as_ref(),
            &site_info,
            query.lang,
        )
        .await
    {
        Ok(identity) => identity,
        Err(err) => return req.error_page(err, query.error_url.as_ref()),
    };

    // 7. Return response
    log::info!("Email flow completed for: {}", identity.id);
    req.error_page(AuthError::EmailLogin, query.error_url.as_ref())
}
