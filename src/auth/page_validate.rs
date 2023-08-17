use crate::{
    auth::{AuthError, AuthPage, AuthServiceState, AuthSession},
    openapi::ApiKind,
};
use axum::{body::HttpBody, extract::State};
use serde::Deserialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, ValidatedQuery, ValidationError},
    service::APP_NAME,
};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    redirect_url: Option<Url>,
}

async fn validate(
    State(state): State<AuthServiceState>,
    auth_session: AuthSession,
    query: Result<ValidatedQuery<Query>, ValidationError>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return state.page_error(auth_session, AuthError::ValidationError(error), None),
    };

    state.page_redirect(auth_session, APP_NAME, query.redirect_url.as_ref())
}

pub fn page_validate<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Page("/auth/validate"), validate)
        .with_operation_id("page_validate")
        .with_tag("page")
        .with_query_parameter::<Query>()
        .with_page_response("Html page to validate cookie consistency. It removes all the invalid cookies and redirect user to the given page")
}
