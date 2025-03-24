use crate::{
    app_state::AppState,
    services::{permissions, IdentityPermissions},
};
use axum::{extract::State, Extension, Json};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use serde::{Deserialize, Serialize};
use shine_infra::web::{
    CheckedCurrentUser, IntoProblemResponse, PermissionError, Problem, ProblemConfig, ProblemResponse, ValidatedJson,
    ValidatedPath,
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PathParams {
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
pub struct AddUserRole {
    #[validate(length(min = 1, max = 63))]
    role: String,
}

#[utoipa::path(
    put,
    path = "/api/identities/{id}/roles",
    tag = "identity",
    params(
        PathParams
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
    ValidatedPath(path): ValidatedPath<PathParams>,
    ValidatedJson(params): ValidatedJson<AddUserRole>,
) -> Result<Json<UserRoles>, ProblemResponse> {
    if let (Some(auth_key), Some(master_key_hash)) = (
        auth_key.map(|auth| auth.token().to_owned()),
        &state.settings().super_user_api_key_hash,
    ) {
        log::trace!("Using api key");
        if !bcrypt::verify(auth_key, master_key_hash).unwrap_or(false) {
            return Err(
                PermissionError::MissingPermission(permissions::UPDATE_ANY_USER_ROLE).into_response(&problem_config)
            );
        }
    } else {
        log::trace!("Using cookie");
        user.identity_permissions()
            .check(permissions::UPDATE_ANY_USER_ROLE)
            .map_err(|err| err.into_response(&problem_config))?;
    }

    state
        .identity_service()
        .add_role(path.user_id, &params.role)
        .await
        .map_err(|err| err.into_response(&problem_config))?
        .ok_or_else(|| {
            Problem::not_found()
                .with_instance_str(format!("{{identity_api}}/identities/{}", path.user_id))
                .into_response(&problem_config)
        })?;

    //todo: make it triggered by the identity
    let (_, roles) = state
        .session_user_handler()
        .refresh_session_user(path.user_id)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(Json(UserRoles { roles }))
}

#[utoipa::path(
    get,
    path = "/api/identities/{id}/roles",
    tag = "identity",
    params(
        PathParams
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
    ValidatedPath(path): ValidatedPath<PathParams>,
) -> Result<Json<UserRoles>, ProblemResponse> {
    if let (Some(auth_key), Some(master_key_hash)) = (
        auth_key.map(|auth| auth.token().to_owned()),
        &state.settings().super_user_api_key_hash,
    ) {
        log::trace!("Using api key");
        if !bcrypt::verify(auth_key, master_key_hash).unwrap_or(false) {
            return Err(
                PermissionError::MissingPermission(permissions::READ_ANY_USER_ROLE).into_response(&problem_config)
            );
        }
    } else {
        log::trace!("Using cookie");
        user.identity_permissions()
            .check(permissions::READ_ANY_USER_ROLE)
            .map_err(|err| err.into_response(&problem_config))?;
    }

    let roles = state
        .identity_service()
        .get_roles(path.user_id)
        .await
        .map_err(|err| err.into_response(&problem_config))?
        .ok_or_else(|| {
            Problem::not_found()
                .with_instance_str(format!("{{identity_api}}/identities/{}", path.user_id))
                .into_response(&problem_config)
        })?;

    Ok(Json(UserRoles { roles }))
}

#[derive(Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "role": "Role"
}))]
pub struct DeleteUserRole {
    #[validate(length(min = 1, max = 32))]
    role: String,
}

#[utoipa::path(
    delete,
    path = "/api/identities/{id}/roles",
    tag = "identity",
    params(
        PathParams
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
    ValidatedPath(path): ValidatedPath<PathParams>,
    ValidatedJson(params): ValidatedJson<DeleteUserRole>,
) -> Result<Json<UserRoles>, ProblemResponse> {
    if let (Some(auth_key), Some(master_key)) = (
        auth_key.map(|auth| auth.token().to_owned()),
        &state.settings().super_user_api_key_hash,
    ) {
        log::trace!("Using api key");
        if !bcrypt::verify(auth_key, master_key).unwrap_or(false) {
            return Err(
                PermissionError::MissingPermission(permissions::UPDATE_ANY_USER_ROLE).into_response(&problem_config)
            );
        }
    } else {
        log::trace!("Using cookie");
        user.identity_permissions()
            .check(permissions::UPDATE_ANY_USER_ROLE)
            .map_err(|err| err.into_response(&problem_config))?;
    }

    state
        .identity_service()
        .delete_role(path.user_id, &params.role)
        .await
        .map_err(|err| err.into_response(&problem_config))?
        .ok_or_else(|| {
            Problem::not_found()
                .with_instance_str(format!("{{identity_api}}/identities/{}", path.user_id))
                .into_response(&problem_config)
        })?;

    //todo: make it triggered by the identity
    let (_, roles) = state
        .session_user_handler()
        .refresh_session_user(path.user_id)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(Json(UserRoles { roles }))
}

#[cfg(test)]
mod test {
    use rand::{distr::Alphanumeric, rng, Rng};

    #[test]
    #[ignore = "This is not a test but a helper to generate master key"]
    fn generate_master_key() {
        let key: String = rng().sample_iter(&Alphanumeric).take(32).map(char::from).collect();

        let hash = bcrypt::hash(&key, 5).unwrap();
        println!("key: {key}");
        println!("hash: {hash}");
    }
}
