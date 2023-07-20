use crate::{
    auth::AuthServiceState,
    db::{DBError, IdentityError},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::Serialize;
use shine_service::service::CurrentUser;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub(in crate::auth) enum Error {
    #[error("User ({0}) not found")]
    UserNotFound(Uuid),
    #[error("User session expired or revoked")]
    SessionExpired,
    #[error(transparent)]
    SessionError(DBError),
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status_code = match &self {
            Error::UserNotFound(_) => StatusCode::UNAUTHORIZED,
            Error::SessionExpired => StatusCode::UNAUTHORIZED,
            Error::SessionError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::IdentityError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::auth) struct UserInfo {
    user_id: Uuid,
    name: String,
    is_email_confirmed: bool,
    session_length: u64,
    roles: Vec<String>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
pub(in crate::auth) async fn ep_get_user_info(
    State(state): State<AuthServiceState>,
    user: CurrentUser,
) -> Result<Json<UserInfo>, Error> {
    let _ = state
        .session_manager()
        .find_session(user.user_id, user.key)
        .await
        .map_err(Error::SessionError)?
        .ok_or(Error::SessionExpired)?;

    let identity = state
        .identity_manager()
        .find(crate::db::FindIdentity::UserId(user.user_id))
        .await?
        .ok_or(Error::UserNotFound(user.user_id))?;

    // use roles from session, as other services will use the same information.
    // let roles = state.identity_manager().get_roles(identity.user_id).await?;

    let session_length = (Utc::now() - user.session_start).num_seconds();
    let session_length = if session_length < 0 { 0 } else { session_length as u64 };
    Ok(Json(UserInfo {
        user_id: user.user_id,
        name: user.name,
        is_email_confirmed: identity.is_email_confirmed,
        session_length,
        roles: user.roles,
    }))
}
