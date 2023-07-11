use crate::auth::{AuthError, AuthPage, AuthServiceState, AuthSession};
use axum::extract::State;
use shine_service::service::APP_NAME;

/// Delete he current user. This is not a soft delete, once executed there is no way back.
/// Note, it only deletes the user and login credentials, but not the data of the user.
pub(in crate::auth) async fn page_delete_user(
    State(state): State<AuthServiceState>,
    mut auth_session: AuthSession,
) -> AuthPage {
    let (user_id, user_key) = match auth_session.user.as_ref().map(|u| (u.user_id, u.key)) {
        Some(user_id) => user_id,
        None => return state.page_error(auth_session, AuthError::LoginRequired),
    };

    // validate session as this is a very risky operation
    match state.session_manager().find_session(user_id, user_key).await {
        Ok(None) => return state.page_error(auth_session, AuthError::SessionExpired),
        Err(err) => return state.page_internal_error(auth_session, err),
        Ok(Some(_)) => {}
    };

    if let Err(err) = state.identity_manager().cascaded_delete(user_id).await {
        return state.page_internal_error(auth_session, err);
    }

    // from this point there is no reason to keep session
    // errors beyond these points are irrelevant for the users and mostly just warnings.
    let _ = auth_session.take();
    if let Err(err) = state.session_manager().remove_all(user_id).await {
        log::warn!("Failed to clear all sessions for user {}: {:?}", user_id, err);
    }

    state.page_redirect(auth_session, APP_NAME, None)
}
