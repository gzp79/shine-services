use crate::{
    app_state::AppState,
    routes::auth::{AuthPage, AuthPageRequest, AuthSession},
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
    /// User confirmation value, it must match the user name to proceed with the deletion.
    confirmation: Option<String>,
    #[param(value_type=Option<String>)]
    redirect_url: Option<Url>,
    #[param(value_type=Option<String>)]
    error_url: Option<Url>,
}

/// Delete he current user. This is not a soft delete, once executed there is no way back.
/// Note, it only deletes the user and login credentials, but not the data of the user.
#[utoipa::path(
    get,
    path = "/auth/delete",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Html page to update clear client cookies and complete user deletion")
    )
)]
pub async fn delete_user(
    State(state): State<AppState>,
    auth_session: AuthSession,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
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

    state
        .delete_user_handler()
        .delete_user(
            req.into_auth_session(),
            query.confirmation.as_deref(),
            query.redirect_url.as_ref(),
            query.error_url.as_ref(),
        )
        .await
}
