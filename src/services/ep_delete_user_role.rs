use crate::{db::Permission, services::IdentityServiceState};
use axum::extract::{Path, State};
use serde::Deserialize;
use shine_service::{
    axum::{Problem, ValidatedJson},
    service::CurrentUser,
};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::services) struct RequestPath {
    #[serde(rename = "id")]
    user_id: Uuid,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub(in crate::services) struct RequestParams {
    #[validate(length(min = 1, max = 32))]
    role: String,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
pub(in crate::services) async fn ep_delete_user_role(
    State(state): State<IdentityServiceState>,
    user: CurrentUser,
    Path(path): Path<RequestPath>,
    ValidatedJson(params): ValidatedJson<RequestParams>,
) -> Result<(), Problem> {
    state.require_permission(&user, Permission::UpdateAnyUserRole).await?;
    state
        .identity_manager()
        .delete_role(path.user_id, &params.role)
        .await
        .map_err(Problem::internal_error_from)?;
    Ok(())
}
