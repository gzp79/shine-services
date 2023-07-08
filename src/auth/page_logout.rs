use crate::{
    auth::{create_ooops_page, create_redirect_page, AuthServiceState, AuthSession},
    db::DBError,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use shine_service::service::{CurrentUser, APP_NAME};

#[derive(Deserialize)]
pub(in crate::auth) struct LogoutRequest {
    terminate_all: Option<bool>,
}

async fn logout_impl(state: &AuthServiceState, user: Option<CurrentUser>, remove_all: bool) -> Result<(), DBError> {
    if let Some(user) = user {
        if remove_all {
            state.session_manager().remove_all(user.user_id).await?;
        } else {
            state.session_manager().remove(user.user_id, user.key).await?;
        }
    }

    Ok(())
}

pub(in crate::auth) async fn logout(
    State(state): State<AuthServiceState>,
    Query(query): Query<LogoutRequest>,
    mut auth_session: AuthSession,
) -> Response {
    log::info!("auth_session: {auth_session:?}");

    let (user, _, _) = auth_session.take();

    match logout_impl(&state, user, query.terminate_all.unwrap_or(false)).await {
        Ok(()) => {
            let html = create_redirect_page(&state, "Redirecting", APP_NAME, None);
            (auth_session, html).into_response()
        }
        Err(err) => {
            let html = create_ooops_page(&state, Some(&format!("{err}")));
            (StatusCode::INTERNAL_SERVER_ERROR, auth_session, html).into_response()
        }
    }
}
