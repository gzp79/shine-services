use crate::{
    auth::{AuthPage, AuthServiceState, AuthSession},
    db::IdentityError,
};
use axum::extract::{Query, State};
use serde::Deserialize;

use super::auth_session::TokenLogin;

#[derive(Deserialize)]
pub(in crate::auth) struct LogoutRequest {
    terminate_all: Option<bool>,
}

// todo: workaround while if let chain is not possible
async fn delete_token(
    state: &AuthServiceState,
    user_id: uuid::Uuid,
    token_login: &Option<TokenLogin>,
) -> Result<(), IdentityError> {
    if let Some(token_login) = token_login {
        state.identity_manager().delete_token(user_id, &token_login.token).await
    } else {
        Ok(())
    }
}

pub(in crate::auth) async fn page_logout(
    State(state): State<AuthServiceState>,
    Query(query): Query<LogoutRequest>,
    mut auth_session: AuthSession,
) -> AuthPage {
    let (user, _, token_login) = auth_session.take();

    if let Some(user) = user {
        match query.terminate_all.unwrap_or(false) {
            false => {
                if let Err(err) = state.identity_manager().delete_all_tokens(user.user_id).await {
                    AuthPage::internal_error(&state, None, err)
                } else if let Err(err) = state.session_manager().remove_all(user.user_id).await {
                    AuthPage::internal_error(&state, Some(auth_session), err)
                } else {
                    AuthPage::redirect(&state, Some(auth_session), None)
                }
            }
            true => {
                if let Err(err) = delete_token(&state, user.user_id, &token_login).await {
                    AuthPage::internal_error(&state, None, err)
                } else if let Err(err) = state.session_manager().remove(user.user_id, user.key).await {
                    AuthPage::internal_error(&state, Some(auth_session), err)
                } else {
                    AuthPage::redirect(&state, Some(auth_session), None)
                }
            }
        }
    } else {
        AuthPage::redirect(&state, Some(auth_session), None)
    }
}
