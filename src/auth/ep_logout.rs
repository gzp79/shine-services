use crate::{
    auth::extern_login_session::ExternalLoginSession,
    db::{DBError, SessionManager, SettingsManager},
};
use axum::{
    extract::Query,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Extension,
};
use serde::Deserialize;
use shine_service::service::{CurrentUser, UserSession};
use std::sync::Arc;
use tera::Tera;

#[derive(Deserialize)]
pub(in crate::auth) struct LogoutRequest {
    terminate_all: Option<bool>,
}

pub(in crate::auth) async fn logout(
    Extension(tera): Extension<Arc<Tera>>,
    Extension(settings): Extension<SettingsManager>,
    Extension(session_manager): Extension<SessionManager>,
    Query(query): Query<LogoutRequest>,
    mut user_session: UserSession,
    mut external_login: ExternalLoginSession,
) -> Response {
    let user_session_data = user_session.take();
    let _ = external_login.take();

    let (status, template, context) = if let Err(err) = perform_logout(
        &session_manager,
        user_session_data,
        query.terminate_all.unwrap_or(false),
    )
    .await
    {
        let mut context = tera::Context::new();
        context.insert("error", &format!("{err:?}"));
        (StatusCode::INTERNAL_SERVER_ERROR, "error.html", context)
    } else {
        let mut context = tera::Context::new();
        context.insert("title", &"Logout");
        context.insert("target", &"home");
        context.insert("redirect_url", settings.home_url());
        (StatusCode::OK, "redirect.html", context)
    };

    // make sure despite of having any server error, the session cookies are cleared
    match tera.render(template, &context) {
        Ok(html) => (status, user_session, external_login, Html(html)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            user_session,
            external_login,
            format!("template error: {err:?}"),
        )
            .into_response(),
    }
}

async fn perform_logout(
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
