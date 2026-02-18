use crate::{
    app_state::AppState,
    controllers::auth::{AuthPage, AuthSession, AuthUtils, PageUtils},
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
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, error.problem, None),
    };
    if let Some(error_url) = &query.error_url {
        if let Err(err) = AuthUtils::new(&state).validate_redirect_url("errorUrl", error_url) {
            return PageUtils::new(&state).error(auth_session, err, None);
        }
    }
    if let Some(redirect_url) = &query.redirect_url {
        if let Err(err) = AuthUtils::new(&state).validate_redirect_url("redirectUrl", redirect_url) {
            return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref());
        }
    }

    log::debug!("Query: {query:#?}");

    PageUtils::new(&state).redirect(auth_session, query.redirect_url.as_ref(), None)
}
