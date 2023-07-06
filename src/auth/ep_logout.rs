use crate::{
    auth::{create_ooops_page, create_redirect_page, extern_login_session::ExternalLoginSession, AuthServiceState},
    db::DBError,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use shine_service::service::{CurrentUser, UserSession, APP_NAME};

#[derive(Deserialize)]
pub(in crate::auth) struct LogoutRequest {
    terminate_all: Option<bool>,
}

async fn logout_impl(
    state: &AuthServiceState,
    current_user: Option<CurrentUser>,
    remove_all: bool,
) -> Result<(), DBError> {
    if let Some(current_user) = current_user {
        if remove_all {
            state.session_manager.remove_all(current_user.user_id).await?;
        } else {
            state
                .session_manager
                .remove(current_user.user_id, current_user.key)
                .await?;
        }
    }

    Ok(())
}

pub(in crate::auth) async fn logout(
    State(state): State<AuthServiceState>,
    Query(query): Query<LogoutRequest>,
    mut user_session: UserSession,
    mut external_login: ExternalLoginSession,
) -> Response {
    let _ = external_login.take();

    match logout_impl(&state, user_session.take(), query.terminate_all.unwrap_or(false)).await {
        Ok(()) => {
            let html = create_redirect_page(&state, "Redirecting", APP_NAME, None);
            (user_session, external_login, html).into_response()
        }
        Err(err) => {
            let html = create_ooops_page(&state, Some(&format!("{err}")));
            (StatusCode::INTERNAL_SERVER_ERROR, user_session, external_login, html).into_response()
        }
    }
}
