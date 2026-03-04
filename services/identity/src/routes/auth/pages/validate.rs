use crate::{
    app_state::AppState,
    routes::auth::{AuthPage, AuthPageRequest, AuthSession, PageUtils},
};
use axum::extract::State;
use serde::Deserialize;
use shine_infra::web::{
    extracts::{InputError, ValidatedQuery},
    responses::ErrorResponse,
};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    #[param(value_type=Option<String>)]
    redirect_url: Option<Url>,
    #[param(value_type=Option<String>)]
    error_url: Option<Url>,
}

#[utoipa::path(
    get,
    path = "/auth/validate",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Html page to validate cookie consistency. It removes all the invalid cookies and redirect user to the given page")
    )
)]
pub async fn validate(
    State(state): State<AppState>,
    auth_session: AuthSession,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
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

    // 4. Return response
    PageUtils::new(&state).redirect(req.into_auth_session(), query.redirect_url.as_ref(), None)
}
