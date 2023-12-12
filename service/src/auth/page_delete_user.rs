use crate::{
    auth::{AuthError, AuthPage, AuthServiceState, AuthSession},
    openapi::ApiKind,
};
use axum::extract::State;
use serde::Deserialize;
use shine_service::axum::{ApiEndpoint, ApiMethod, InputError, ValidatedQuery};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    /// Set to true. Mainly used to avoid some accidental automated deletion.
    /// It is suggested to have some confirmation on the UI (for example enter the name of the user to be deleted) and
    /// set the value of the property to the result of the confirmation.
    confirmed: bool,
    redirect_url: Option<Url>,
    error_url: Option<Url>,
}

/// Delete he current user. This is not a soft delete, once executed there is no way back.
/// Note, it only deletes the user and login credentials, but not the data of the user.
async fn delete_user(
    State(state): State<AuthServiceState>,
    mut auth_session: AuthSession,
    query: Result<ValidatedQuery<Query>, InputError>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return state.page_error(auth_session, AuthError::InputError(error), None),
    };

    let (user_id, user_key) = match auth_session.user_session.as_ref().map(|u| (u.user_id, u.key)) {
        Some(user_id) => user_id,
        None => return state.page_error(auth_session, AuthError::LoginRequired, query.error_url.as_ref()),
    };

    // some agting mainly used for swagger ui
    if !query.confirmed {
        return state.page_error(auth_session, AuthError::MissingPrecondition, query.error_url.as_ref());
    }

    // validate session as this is a very risky operation
    match state.session_manager().find(user_id, user_key).await {
        Ok(None) => return state.page_error(auth_session, AuthError::SessionExpired, query.error_url.as_ref()),
        Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
        Ok(Some(_)) => {}
    };

    if let Err(err) = state.identity_manager().cascaded_delete(user_id).await {
        return state.page_internal_error(auth_session, err, query.error_url.as_ref());
    }

    // from this point there is no reason to keep session
    // errors beyond these points are irrelevant for the users and mostly just warnings.
    auth_session.clear();
    if let Err(err) = state.session_manager().remove_all(user_id).await {
        log::warn!("Failed to clear all sessions for user {}: {:?}", user_id, err);
    }

    state.page_redirect(auth_session, state.app_name(), query.redirect_url.as_ref())
}

pub fn page_delete_user() -> ApiEndpoint<AuthServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Page("/auth/delete"), delete_user)
        .with_operation_id("page_delete_user")
        .with_tag("page")
        .with_query_parameter::<Query>()
        .with_page_response("Html page to update clear client cookies and complete user deletion")
}
