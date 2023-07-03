use crate::{
    auth::{create_ooops_page, create_redirect_page, extern_login_session::ExternalLoginSession},
    db::{DBError, SessionManager, SettingsManager},
};
use axum::{extract::Query, http::StatusCode, response::{Response, IntoResponse}, Extension};
use serde::Deserialize;
use shine_service::service::{CurrentUser, UserSession, APP_NAME};
use std::sync::Arc;
use tera::Tera;

#[derive(Deserialize)]
pub(in crate::auth) struct LogoutRequest {
    terminate_all: Option<bool>,
}

async fn logout_impl(
    session_manager: &SessionManager,
    current_user: Option<CurrentUser>,
    remove_all: bool,
) -> Result<(), DBError> {
    if let Some(current_user) = current_user {
        if remove_all {
            session_manager.remove_all(current_user.user_id).await?;
        } else {
            session_manager.remove(current_user.user_id, current_user.key).await?;
        }
    }

    Ok(())
}

pub(in crate::auth) async fn logout(
    Extension(tera): Extension<Arc<Tera>>,
    Extension(settings_manager): Extension<SettingsManager>,
    Extension(session_manager): Extension<SessionManager>,
    Query(query): Query<LogoutRequest>,
    mut user_session: UserSession,
    mut external_login: ExternalLoginSession,
) -> Response {
    let _ = external_login.take();

    match logout_impl(
        &session_manager,
        user_session.take(),
        query.terminate_all.unwrap_or(false),
    )
    .await
    {
        Ok(()) => {
            let html = create_redirect_page(&tera, &settings_manager, "Redirecting", APP_NAME, None);
            (user_session, external_login, html).into_response()
        }
        Err(err) => {
            let html = create_ooops_page(&tera, &settings_manager, Some(format!("{err}")));
            (StatusCode::INTERNAL_SERVER_ERROR, user_session, external_login, html).into_response()
        }
    }
}
