use crate::{
    db::{IdentityError, Permission, PermissionError},
    services::{GetPermissionError, IdentityServiceState},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use shine_service::service::CurrentUser;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub(in crate::services) enum Error {
    #[error("Missing role")]
    PermissionError(#[from] PermissionError),
    #[error(transparent)]
    GetPermissionError(#[from] GetPermissionError),
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status_code = match &self {
            Error::PermissionError(PermissionError::MissingPermission(_)) => StatusCode::FORBIDDEN,
            Error::GetPermissionError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::IdentityError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::services) struct RequestPath {
    #[serde(rename = "id")]
    user_id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::services) struct RequestParams {
    role: String,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
pub(in crate::services) async fn ep_delete_user_role(
    State(state): State<IdentityServiceState>,
    user: CurrentUser,
    Path(path): Path<RequestPath>,
    Json(params): Json<RequestParams>,
) -> Result<(), Error> {
    state
        .get_permissions(&user)
        .await?
        .require(Permission::UpdateUserRole)?;
    state.identity_manager().delete_role(path.user_id, &params.role).await?;
    Ok(())
}
