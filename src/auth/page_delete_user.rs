use crate::auth::{create_ooops_page, create_redirect_page, AuthSession};
use crate::{
    auth::AuthServiceState,
    db::{DBError, IdentityError},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use shine_service::service::{CurrentUser, APP_NAME};
use thiserror::Error as ThisError;

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

/// Delete he current user. This is not a soft delete, once executed there is no way back.
/// Note, it only deletes the user and login credentials, but not the data of the user.
pub(in crate::auth) async fn user_delete_impl(
    state: &AuthServiceState,
    user: Option<CurrentUser>,
) -> Result<(), Error> {
    let user = user.ok_or(Error::SessionExpired)?;

    if state
        .session_manager()
        .find_session(user.user_id, user.key)
        .await?
        .is_none()
    {
        return Err(Error::SessionExpired);
    }

    state.identity_manager().delete_identity(user.user_id).await?;
    state.session_manager().remove_all(user.user_id).await?;

    Ok(())
}

pub(in crate::auth) async fn user_delete(
    State(state): State<AuthServiceState>,
    mut auth_session: AuthSession,
) -> Response {
    let (user, _, _) = auth_session.take();

    match user_delete_impl(&state, user).await {
        Ok(()) => {
            let html = create_redirect_page(&state, "Redirecting", APP_NAME, None);
            (auth_session, html).into_response()
        }
        err @ Err(Error::SessionExpired) => {
            log::info!("Session is corrupted: {err:?}");
            let html = create_ooops_page(&state, Some("Session is corrupted, clearing stored sessions"));
            let _ = auth_session.take();
            (StatusCode::FORBIDDEN, auth_session, html).into_response()
        }
        Err(err) => {
            let html = create_ooops_page(&state, Some(&format!("{err}")));
            (StatusCode::INTERNAL_SERVER_ERROR, html).into_response()
        }
    }
}
