use crate::{auth::AuthServiceState, db::DBError};
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
    #[error("User session has expired")]
    SessionExpired,

    #[error(transparent)]
    DBError(#[from] DBError),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status_code = match &self {
            Error::UserNotFound(_) => StatusCode::NOT_FOUND,
            Error::SessionExpired => StatusCode::UNAUTHORIZED,
            Error::DBError(_) => StatusCode::INTERNAL_SERVER_ERROR,
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
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
/// This action also check for cookie expiration and revoke.
pub(in crate::auth) async fn user_info(
    State(state): State<AuthServiceState>,
    current_user: CurrentUser,
) -> Result<Json<UserInfo>, Error> {
    if state
        .session_manager()
        .find_session(current_user.user_id, current_user.key)
        .await?
        .is_none()
    {
        return Err(Error::SessionExpired);
    }

    let identity = state
        .identity_manager()
        .find(crate::db::FindIdentity::UserId(current_user.user_id))
        .await?
        .ok_or(Error::UserNotFound(current_user.user_id))?;

    let session_length = (Utc::now() - current_user.session_start).num_seconds();
    let session_length = if session_length < 0 { 0 } else { session_length as u64 };
    Ok(Json(UserInfo {
        user_id: current_user.user_id,
        name: current_user.name,
        is_email_confirmed: identity.is_email_confirmed,
        session_length,
    }))
}
