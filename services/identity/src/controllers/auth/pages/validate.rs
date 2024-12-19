use crate::controllers::{
    auth::{AuthError, AuthPage, AuthSession, PageUtils},
    ApiKind, AppState,
};
use axum::extract::State;
use serde::Deserialize;
use shine_service::axum::{ApiEndpoint, ApiMethod, ConfiguredProblem, InputError, ValidatedQuery};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct QueryParams {
    #[param(value_type=Option<String>)]
    redirect_url: Option<Url>,
}

async fn validate(
    State(state): State<AppState>,
    auth_session: AuthSession,
    query: Result<ValidatedQuery<QueryParams>, ConfiguredProblem<InputError>>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, AuthError::InputError(error.problem), None),
    };

    PageUtils::new(&state).redirect(auth_session, None, query.redirect_url.as_ref())
}

pub fn page_validate() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Page("/auth/validate"), validate)
        .with_operation_id("validate")
        .with_tag("page")
        .with_query_parameter::<QueryParams>()
        .with_page_response("Html page to validate cookie consistency. It removes all the invalid cookies and redirect user to the given page")
}
