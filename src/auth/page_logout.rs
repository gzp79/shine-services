use crate::auth::{AuthPage, AuthServiceState, AuthSession};
use axum::extract::{Query, State};
use serde::Deserialize;

#[derive(Deserialize)]
pub(in crate::auth) struct LogoutRequest {
    terminate_all: Option<bool>,
}

pub(in crate::auth) async fn page_logout(
    State(state): State<AuthServiceState>,
    Query(query): Query<LogoutRequest>,
    mut auth_session: AuthSession,
) -> AuthPage {
    let (user, _, _) = auth_session.take();

    if let Some(user) = user {
        let result = if query.terminate_all.unwrap_or(false) {
            state.session_manager().remove_all(user.user_id).await
        } else {
            state.session_manager().remove(user.user_id, user.key).await
        };

        match result {
            Ok(_) => AuthPage::redirect(&state, auth_session, None),
            Err(err) => AuthPage::internal_error(&state, None, err),
        }
    } else {
        AuthPage::redirect(&state, auth_session, None)
    }
}
