use crate::{
    identity::IdentityServiceState,
    openapi::ApiKind,
    repositories::{Permission, PermissionError, Role},
};
use axum::{
    body::HttpBody,
    extract::State,
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    BoxError, Json, TypedHeader,
};
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
    #[validate(length(min = 1, max = 63))]
    role: String,
}

async fn add_user_role(
    State(state): State<IdentityServiceState>,
    user: CheckedCurrentUser,
    auth_key: Option<TypedHeader<Authorization<Bearer>>>,
    ValidatedPath(path): ValidatedPath<Path>,
    ValidatedJson(params): ValidatedJson<AddUserRole>,
) -> Result<Json<UserRoles>, Problem> {
    if let (Some(auth_key), Some(master_key_hash)) = (
        auth_key.map(|auth| auth.token().to_owned()),
        state.master_api_key_hash(),
    ) {
        log::trace!("Using api key");
        if !bcrypt::verify(auth_key, master_key_hash).unwrap_or(false) {
            return Err(PermissionError::MissingPermission(Permission::UpdateAnyUserRole).into());
        }
    } else {
        log::trace!("Using cookie");
        state.require_permission(&user, Permission::UpdateAnyUserRole).await?;
    }

    state
        .identity_manager()
        .add_role(path.user_id, &params.role)
        .await
        .map_err(Problem::internal_error_from)?
        .ok_or_else(|| Problem::not_found().with_instance(format!("{{identity_api}}/identities/{}", path.user_id)))?;

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
    auth_key: Option<TypedHeader<Authorization<Bearer>>>,
    ValidatedPath(path): ValidatedPath<Path>,
) -> Result<Json<UserRoles>, Problem> {
    if let (Some(auth_key), Some(master_key_hash)) = (
        auth_key.map(|auth| auth.token().to_owned()),
        state.master_api_key_hash(),
    ) {
        log::trace!("Using api key");
        if !bcrypt::verify(auth_key, master_key_hash).unwrap_or(false) {
            return Err(PermissionError::MissingPermission(Permission::ReadAnyUserRole).into());
        }
    } else {
        log::trace!("Using cookie");
        state.require_permission(&user, Permission::ReadAnyUserRole).await?;
    }

    let roles = state
        .identity_manager()
        .get_roles(path.user_id)
        .await
        .map_err(Problem::internal_error_from)?
        .ok_or_else(|| Problem::not_found().with_instance(format!("{{identity_api}}/identities/{}", path.user_id)))?;

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
    auth_key: Option<TypedHeader<Authorization<Bearer>>>,
    ValidatedPath(path): ValidatedPath<Path>,
    ValidatedJson(params): ValidatedJson<DeleteUserRole>,
) -> Result<Json<UserRoles>, Problem> {
    if let (Some(auth_key), Some(master_key)) = (
        auth_key.map(|auth| auth.token().to_owned()),
        state.master_api_key_hash(),
    ) {
        log::trace!("Using api key");
        if !bcrypt::verify(auth_key, master_key).unwrap_or(false) {
            return Err(PermissionError::MissingPermission(Permission::UpdateAnyUserRole).into());
        }
    } else {
        log::trace!("Using cookie");
        state.require_permission(&user, Permission::UpdateAnyUserRole).await?;
    }

    state
        .identity_manager()
        .delete_role(path.user_id, &params.role)
        .await
        .map_err(Problem::internal_error_from)?
        .ok_or_else(|| Problem::not_found().with_instance(format!("{{identity_api}}/identities/{}", path.user_id)))?;

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

#[cfg(test)]
mod test {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    #[test]
    #[ignore = "This is not a test but a helper to generate master key"]
    fn generate_master_key() {
        let key: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let hash = bcrypt::hash(&key, 5).unwrap();
        println!("key: {key}");
        println!("hash: {hash}");
    }
}
