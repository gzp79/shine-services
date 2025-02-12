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
pub struct QueryParams {
    /// Set to true. Mainly used to avoid some accidental automated deletion.
    /// It is suggested to have some confirmation on the UI (for example enter the name of the user to be deleted) and
    /// set the value of the property to the result of the confirmation.
    confirmed: bool,
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
    query: Result<ValidatedQuery<QueryParams>, ConfiguredProblem<InputError>>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, AuthError::InputError(error.problem), None),
    };

    let (user_id, session_key) = match auth_session.user_session().map(|u| (u.user_id, u.key)) {
        Some(user_id) => user_id,
        None => return PageUtils::new(&state).error(auth_session, AuthError::LoginRequired, query.error_url.as_ref()),
    };

    // some gating mainly used in the swagger ui not to accidentally delete the user
    if !query.confirmed {
        return PageUtils::new(&state).error(auth_session, AuthError::MissingPrecondition, query.error_url.as_ref());
    }

    // validate session as this is a very risky operation
    match state.session_service().find(user_id, &session_key).await {
        Ok(None) => {
            return PageUtils::new(&state).error(auth_session, AuthError::SessionExpired, query.error_url.as_ref())
        }
        Err(err) => return PageUtils::new(&state).internal_error(auth_session, err, query.error_url.as_ref()),
        Ok(Some(_)) => {}
    };

    if let Err(err) = state.identity_service().cascaded_delete(user_id).await {
        return PageUtils::new(&state).internal_error(auth_session, err, query.error_url.as_ref());
    }

    // End of validations, from this point
    //  - there is no reason to keep session
    //  - errors are irrelevant for the users and mostly just warnings.
    let response_session = auth_session.cleared();

    if let Err(err) = state.session_service().remove_all(user_id).await {
        log::warn!("Failed to clear all sessions for user {}: {:?}", user_id, err);
    }

    PageUtils::new(&state).redirect(response_session, None, query.redirect_url.as_ref())
}
