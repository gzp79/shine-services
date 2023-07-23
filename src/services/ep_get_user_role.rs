use crate::{db::Permission, services::IdentityServiceState};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use shine_service::{axum::Problem, service::CurrentUser};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::services) struct RequestPath {
    #[serde(rename = "id")]
    user_id: Uuid,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    roles: Vec<String>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
pub(in crate::services) async fn ep_get_user_roles(
    State(state): State<IdentityServiceState>,
    user: CurrentUser,
    Path(path): Path<RequestPath>,
) -> Result<Json<Response>, Problem> {
    state.require_permission(&user, Permission::ReadAnyUserRole).await?;
    let roles = state
        .identity_manager()
        .get_roles(path.user_id)
        .await
        .map_err(Problem::internal_error_from)?;
    Ok(Json(Response { roles }))
}
