use crate::{db::Permission, openapi::ApiKind, services::IdentityServiceState};
use axum::{
    body::HttpBody,
    extract::{Path, State},
    BoxError,
};
use serde::Deserialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, ValidatedJson},
    service::CurrentUser,
};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestPath {
    #[serde(rename = "id")]
    user_id: Uuid,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
struct RequestParams {
    #[validate(length(min = 1, max = 32))]
    role: String,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
async fn delete_user_role(
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

pub fn ep_delete_user_role<B>() -> ApiEndpoint<IdentityServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(
        ApiMethod::Delete,
        ApiKind::Api("/identities/:id/roles"),
        delete_user_role,
    )
    .with_operation_id("ep_delete_user_role")
    .with_tag("identity")
}
