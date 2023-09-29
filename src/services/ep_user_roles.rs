use crate::{
    db::{Permission, Role},
    openapi::ApiKind,
    services::IdentityServiceState,
};
use axum::{body::HttpBody, extract::State, http::StatusCode, BoxError, Json};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, ValidatedJson, ValidatedPath},
    service::CheckedCurrentUser,
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Path {
    #[serde(rename = "id")]
    user_id: Uuid,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "roles": ["Role1", "Role2"]
}))]
pub struct UserRoles {
    roles: Vec<Role>,
}

#[derive(Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "role": "Role"
}))]
struct AddUserRole {
    #[validate(length(min = 1, max = 32))]
    role: String,
}

async fn add_user_role(
    State(state): State<IdentityServiceState>,
    user: CheckedCurrentUser,
    ValidatedPath(path): ValidatedPath<Path>,
    ValidatedJson(params): ValidatedJson<AddUserRole>,
) -> Result<Json<UserRoles>, Problem> {
    state.require_permission(&user, Permission::UpdateAnyUserRole).await?;
    state
        .identity_manager()
        .add_role(path.user_id, &params.role)
        .await
        .map_err(Problem::internal_error_from)?;
    let (_, roles) = state.update_session(path.user_id).await?;
    Ok(Json(UserRoles { roles }))
}

pub fn ep_add_user_role<B>() -> ApiEndpoint<IdentityServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Put, ApiKind::Api("/identities/:id/roles"), add_user_role)
        .with_operation_id("ep_add_user_role")
        .with_tag("identity")
        //.with_required_user([Permission::UpdateAnyUserRole])
        .with_path_parameter::<Path>()
        .with_json_request::<AddUserRole>()
        .with_json_response::<UserRoles>(StatusCode::OK)
    //.with_problem_response()
}

async fn get_user_roles(
    State(state): State<IdentityServiceState>,
    user: CheckedCurrentUser,
    ValidatedPath(path): ValidatedPath<Path>,
) -> Result<Json<UserRoles>, Problem> {
    state.require_permission(&user, Permission::ReadAnyUserRole).await?;
    let roles = state
        .identity_manager()
        .get_roles(path.user_id)
        .await
        .map_err(Problem::internal_error_from)?;
    Ok(Json(UserRoles { roles }))
}

pub fn ep_get_user_roles<B>() -> ApiEndpoint<IdentityServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/identities/:id/roles"), get_user_roles)
        .with_operation_id("ep_get_user_roles")
        .with_tag("identity")
        //.with_required_user([Permission::ReadAnyUserRole])
        .with_path_parameter::<Path>()
        .with_json_response::<UserRoles>(StatusCode::OK)
    //.with_problem_response()
}

#[derive(Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "role": "Role"
}))]
struct DeleteUserRole {
    #[validate(length(min = 1, max = 32))]
    role: String,
}

async fn delete_user_role(
    State(state): State<IdentityServiceState>,
    user: CheckedCurrentUser,
    ValidatedPath(path): ValidatedPath<Path>,
    ValidatedJson(params): ValidatedJson<DeleteUserRole>,
) -> Result<Json<UserRoles>, Problem> {
    state.require_permission(&user, Permission::UpdateAnyUserRole).await?;
    state
        .identity_manager()
        .delete_role(path.user_id, &params.role)
        .await
        .map_err(Problem::internal_error_from)?;
    let (_, roles) = state.update_session(path.user_id).await?;
    Ok(Json(UserRoles { roles }))
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
    //.with_required_user([Permission::UpdateAnyUserRole])
    .with_path_parameter::<Path>()
    .with_json_request::<DeleteUserRole>()
    .with_json_response::<UserRoles>(StatusCode::OK)
    //.with_problem_response()
}
