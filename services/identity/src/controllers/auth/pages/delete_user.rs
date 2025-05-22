use crate::{
    app_state::AppState,
    controllers::auth::{AuthError, AuthPage, AuthSession, PageUtils},
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

#[derive(Deserialize, Validate, IntoParams)]
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
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, error.problem, None, None),
    };

    let (user_id, user_name, session_key) =
        match auth_session.user_session().map(|u| (u.user_id, u.name.clone(), u.key)) {
            Some(user) => user,
            None => {
                return PageUtils::new(&state).error(
                    auth_session,
                    AuthError::LoginRequired,
                    query.error_url.as_ref(),
                    query.redirect_url.as_ref(),
                )
            }
        };

    // check for user confirmation
    if query.confirmation != Some(user_name) {
        return PageUtils::new(&state).error(
            auth_session,
            AuthError::MissingConfirmation,
            query.error_url.as_ref(),
            query.redirect_url.as_ref(),
        );
    }

    // validate session as this is a very risky operation
    match state.session_service().find(user_id, &session_key).await {
        Ok(None) => {
            return PageUtils::new(&state).error(
                auth_session,
                AuthError::SessionExpired,
                query.error_url.as_ref(),
                query.redirect_url.as_ref(),
            )
        }
        Err(err) => {
            return PageUtils::new(&state).error(
                auth_session,
                err,
                query.error_url.as_ref(),
                query.redirect_url.as_ref(),
            )
        }
        Ok(Some(_)) => {}
    };

    if let Err(err) = state.identity_service().cascaded_delete(user_id).await {
        return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref(), query.redirect_url.as_ref());
    }

    // End of validations, from this point
    //  - there is no reason to keep session
    //  - errors are irrelevant for the users and mostly just warnings.
    let response_session = auth_session.cleared();

    if let Err(err) = state.session_service().remove_all(user_id).await {
        log::warn!("Failed to clear all sessions for user {}: {:?}", user_id, err);
    }

    PageUtils::new(&state).redirect(response_session, query.redirect_url.as_ref(), None)
}
