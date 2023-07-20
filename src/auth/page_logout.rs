use crate::auth::{AuthPage, AuthServiceState, AuthSession};
use axum::extract::{Query, State};
use serde::Deserialize;
use shine_service::service::APP_NAME;
use url::Url;

#[derive(Deserialize)]
pub(in crate::auth) struct RequestQuery {
    terminate_all: Option<bool>,
    redirect_url: Option<Url>,
    error_url: Option<Url>,
}

pub(in crate::auth) async fn page_logout(
    State(state): State<AuthServiceState>,
    Query(query): Query<RequestQuery>,
    mut auth_session: AuthSession,
) -> AuthPage {
    if let Some((user_id, user_key)) = auth_session.user.as_ref().map(|u| (u.user_id, u.key)) {
        match query.terminate_all.unwrap_or(false) {
            false => {
                if let Err(err) = state.identity_manager().delete_all_tokens(user_id).await {
                    return state.page_internal_error(auth_session, err, query.error_url.as_ref());
                }

                // from this point there is no reason to keep session
                // errors beyond these points are irrelevant for the users and mostly just warnings.
                auth_session.clear();
                if let Err(err) = state.session_manager().remove_all(user_id).await {
                    log::warn!("Failed to clear all sessions for user {}: {:?}", user_id, err);
                }
            }
            true => {
                if let Some(token) = auth_session.token_login.as_ref().map(|t| t.token.clone()) {
                    if let Err(err) = state.identity_manager().delete_token(user_id, &token).await {
                        return state.page_internal_error(auth_session, err, query.error_url.as_ref());
                    }
                }

                // from this point there is no reason to keep session
                // errors beyond these points are irrelevant for the users and mostly just warnings.
                auth_session.clear();
                if let Err(err) = state.session_manager().remove(user_id, user_key).await {
                    log::warn!("Failed to clear session for user {}: {:?}", user_id, err);
                }
            }
        };
    }

    state.page_redirect(auth_session, APP_NAME, query.redirect_url.as_ref())
}
