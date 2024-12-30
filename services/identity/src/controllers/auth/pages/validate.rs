use crate::{
    app_state::AppState,
    controllers::auth::{AuthError, AuthPage, AuthSession, PageUtils},
};
use axum::extract::State;
use serde::Deserialize;
use shine_core::web::{ConfiguredProblem, InputError, ValidatedQuery};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct QueryParams {
    #[param(value_type=Option<String>)]
    redirect_url: Option<Url>,
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
    query: Result<ValidatedQuery<QueryParams>, ConfiguredProblem<InputError>>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, AuthError::InputError(error.problem), None),
    };

    PageUtils::new(&state).redirect(auth_session, None, query.redirect_url.as_ref())
}
