use crate::{
    app_state::AppState,
    handlers::{AuthenticationSuccess, TokenIssuance},
    routes::auth::{AuthPage, AuthPageRequest, AuthSession},
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
    let req = AuthPageRequest::new(&state, auth_session);

    let query = match req.validate_query(query) {
        Ok(q) => q,
        Err(page) => return page,
    };

    if let Some(page) = req.validate_redirect_urls(query.redirect_url.as_ref(), query.error_url.as_ref()) {
        return page;
    }

    log::debug!("Query: {query:#?}");

    let auth_session = req.into_auth_session().with_external_login(None);

    let AuthenticationSuccess {
        identity,
        create_access_token,
        auth_session,
        rotated_token,
    } = match state
        .auth_handler()
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

    let issuance = match (create_access_token, rotated_token) {
        (false, _) => TokenIssuance::Skip,
        (true, None) => TokenIssuance::Create,
        (true, Some(t)) => TokenIssuance::Rotate(t),
    };

    state
        .credential_handler()
        .establish(
            identity,
            issuance,
            auth_session,
            &fingerprint,
            &site_info,
            query.redirect_url.as_ref(),
            query.error_url.as_ref(),
        )
        .await
}
