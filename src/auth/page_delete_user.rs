use crate::auth::{AuthPage, AuthServiceState, AuthSession};
use axum::extract::State;

/// Delete he current user. This is not a soft delete, once executed there is no way back.
/// Note, it only deletes the user and login credentials, but not the data of the user.
pub(in crate::auth) async fn page_delete_user(
    State(state): State<AuthServiceState>,
    mut auth_session: AuthSession,
) -> AuthPage {
    let (user, _, _) = auth_session.take();

    if let Some(user) = user {
        match state.session_manager().find_session(user.user_id, user.key).await {
            Ok(Some(_)) => {}
            Ok(None) => return AuthPage::invalid_session_logout(&state, auth_session),
            Err(err) => return AuthPage::internal_error(&state, None, err),
        };

        if let Err(err) = state.identity_manager().delete_identity(user.user_id).await {
            AuthPage::internal_error(&state, None, err)
        } else if let Err(err) = state.session_manager().remove_all(user.user_id).await {
            AuthPage::internal_error(&state, None, err)
        } else {
            AuthPage::redirect(&state, auth_session, None)
        }
    } else {
        AuthPage::invalid_session_logout(&state, auth_session)
    }
}
