use crate::{
    auth::AuthServiceState,
    db::{DBError, IdentityError},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use shine_service::service::{CurrentUser, UserSession, APP_NAME};
use thiserror::Error as ThisError;

use super::{create_ooops_page, create_redirect_page, extern_login_session::ExternalLoginSession};

#[derive(Debug, ThisError)]
pub(in crate::auth) enum Error {
    #[error("User session has expired")]
    SessionExpired,
    #[error(transparent)]
    IdentityError(#[from] IdentityError),

    #[error(transparent)]
    DBError(#[from] DBError),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status_code = match &self {
            Error::SessionExpired => StatusCode::UNAUTHORIZED,
            Error::IdentityError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::DBError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}

/// Delete he current user. This is not a soft delet, once executed there is no way back.
/// Note, it only deletes the user and llogin credentials, but not the data of the user.
pub(in crate::auth) async fn user_delete_impl(
    state: &AuthServiceState,
    current_user: Option<CurrentUser>,
) -> Result<(), Error> {
    let current_user = current_user.ok_or(Error::SessionExpired)?;
    if state
        .session_manager()
        .find_session(current_user.user_id, current_user.key)
        .await?
        .is_none()
    {
        return Err(Error::SessionExpired);
    }

    state.identity_manager().delete_identity(current_user.user_id).await?;
    state.session_manager().remove_all(current_user.user_id).await?;

    Ok(())
}

pub(in crate::auth) async fn user_delete(
    State(state): State<AuthServiceState>,
    mut user_session: UserSession,
    mut external_login: ExternalLoginSession,
) -> Response {
    let _ = external_login.take();
    match user_delete_impl(&state, user_session.take()).await {
        Ok(()) => {
            let html = create_redirect_page(&state, "Redirecting", APP_NAME, None);
            // delete both the user and external login sessions
            (user_session, external_login, html).into_response()
        }
        Err(err) => {
            let html = create_ooops_page(&state, Some(&format!("{err}")));
            // keep user session, but delete any external login
            (StatusCode::INTERNAL_SERVER_ERROR, external_login, html).into_response()
        }
    }
}
