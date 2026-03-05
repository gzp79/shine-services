use crate::{
    app_state::AppState,
    routes::auth::{AuthError, AuthPage, AuthPageRequest, AuthSession, PageUtils},
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

    // 4. Business logic - delete user
    let (user_id, user_name, session_key) = match req
        .auth_session()
        .user_session()
        .map(|u| (u.user_id, u.name.clone(), u.key))
    {
        Some(user) => user,
        None => return req.error_page(AuthError::LoginRequired, query.error_url.as_ref()),
    };

    // check for user confirmation
    if query.confirmation != Some(user_name) {
        return req.error_page(AuthError::MissingConfirmation, query.error_url.as_ref());
    }

    // validate session as this is a very risky operation
    match req.state().session_service().find(user_id, &session_key).await {
        Ok(None) => return req.error_page(AuthError::SessionExpired, query.error_url.as_ref()),
        Err(err) => return req.error_page(err, query.error_url.as_ref()),
        Ok(Some(_)) => {}
    };

    if let Err(err) = req.state().user_service().delete(user_id).await {
        return req.error_page(err, query.error_url.as_ref());
    }

    // End of validations, from this point
    //  - there is no reason to keep session
    //  - errors are irrelevant for the users and mostly just warnings.
    let response_session = req.into_auth_session().cleared();

    if let Err(err) = state.session_service().remove_all(user_id).await {
        log::warn!("Failed to clear all sessions for user {user_id}: {err:?}");
    }

    // 5. Return response
    PageUtils::new(&state).redirect(response_session, query.redirect_url.as_ref(), None)
}
