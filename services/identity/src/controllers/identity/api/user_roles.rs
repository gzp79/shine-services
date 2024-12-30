use crate::{
    app_state::AppState,
    services::{Permission, PermissionError},
};
use axum::{extract::State, Extension, Json};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use serde::{Deserialize, Serialize};
use shine_core::{
    service::CheckedCurrentUser,
    web::{IntoProblem, Problem, ProblemConfig, ValidatedJson, ValidatedPath},
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
    roles: Vec<String>,
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

#[utoipa::path(
    put,
    path = "/api/identities/:id/roles",
    tag = "identity",
    params(
        Path
    ),
    request_body = AddUserRole,
    responses(
        (status = OK, body = UserRoles),
        //(status = BAD_REQUEST, body = Problem)
    )
)]
pub async fn add_user_role(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    auth_key: Option<TypedHeader<Authorization<Bearer>>>,
    ValidatedPath(path): ValidatedPath<Path>,
    ValidatedJson(params): ValidatedJson<AddUserRole>,
) -> Result<Json<UserRoles>, Problem> {
    if let (Some(auth_key), Some(master_key_hash)) = (
        auth_key.map(|auth| auth.token().to_owned()),
        &state.settings().super_user_api_key_hash,
    ) {
        log::trace!("Using api key");
        if !bcrypt::verify(auth_key, master_key_hash).unwrap_or(false) {
            return Err(PermissionError::MissingPermission(Permission::UpdateAnyUserRole).into_problem(&problem_config));
        }
    } else {
        log::trace!("Using cookie");
        state.check_permission(&user, Permission::UpdateAnyUserRole).await?;
    }

    state
        .identity_service()
        .add_role(path.user_id, &params.role)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to add role", err))?
        .ok_or_else(|| {
            Problem::not_found().with_instance_str(format!("{{identity_api}}/identities/{}", path.user_id))
        })?;

    let (_, roles) = state
        .session_user_sync_service()
        .refresh_session_user(path.user_id)
        .await
        .map_err(|err| err.into_problem(&problem_config))?;

    Ok(Json(UserRoles { roles }))
}

#[utoipa::path(
    get,
    path = "/api/identities/:id/roles",
    tag = "identity",
    params(
        Path
    ),
    responses(
        (status = OK, body = UserRoles),
        //(status = BAD_REQUEST, body = Problem)
    )
)]
pub async fn get_user_roles(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    auth_key: Option<TypedHeader<Authorization<Bearer>>>,
    ValidatedPath(path): ValidatedPath<Path>,
) -> Result<Json<UserRoles>, Problem> {
    if let (Some(auth_key), Some(master_key_hash)) = (
        auth_key.map(|auth| auth.token().to_owned()),
        &state.settings().super_user_api_key_hash,
    ) {
        log::trace!("Using api key");
        if !bcrypt::verify(auth_key, master_key_hash).unwrap_or(false) {
            return Err(PermissionError::MissingPermission(Permission::ReadAnyUserRole).into_problem(&problem_config));
        }
    } else {
        log::trace!("Using cookie");
        state.check_permission(&user, Permission::ReadAnyUserRole).await?;
    }

    let roles = state
        .identity_service()
        .get_roles(path.user_id)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to get roles", err))?
        .ok_or_else(|| {
            Problem::not_found().with_instance_str(format!("{{identity_api}}/identities/{}", path.user_id))
        })?;

    Ok(Json(UserRoles { roles }))
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

#[utoipa::path(
    delete,
    path = "/api/identities/:id/roles",
    tag = "identity",
    params(
        Path
    ),
    request_body = DeleteUserRole,
    responses(
        (status = OK, body = UserRoles),
        //(status = BAD_REQUEST, body = Problem)
    )
)]
pub async fn delete_user_role(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    auth_key: Option<TypedHeader<Authorization<Bearer>>>,
    ValidatedPath(path): ValidatedPath<Path>,
    ValidatedJson(params): ValidatedJson<DeleteUserRole>,
) -> Result<Json<UserRoles>, Problem> {
    if let (Some(auth_key), Some(master_key)) = (
        auth_key.map(|auth| auth.token().to_owned()),
        &state.settings().super_user_api_key_hash,
    ) {
        log::trace!("Using api key");
        if !bcrypt::verify(auth_key, master_key).unwrap_or(false) {
            return Err(PermissionError::MissingPermission(Permission::UpdateAnyUserRole).into_problem(&problem_config));
        }
    } else {
        log::trace!("Using cookie");
        state.check_permission(&user, Permission::UpdateAnyUserRole).await?;
    }

    state
        .identity_service()
        .delete_role(path.user_id, &params.role)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to delete role", err))?
        .ok_or_else(|| {
            Problem::not_found().with_instance_str(format!("{{identity_api}}/identities/{}", path.user_id))
        })?;

    let (_, roles) = state
        .session_user_sync_service()
        .refresh_session_user(path.user_id)
        .await
        .map_err(|err| err.into_problem(&problem_config))?;

    Ok(Json(UserRoles { roles }))
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
