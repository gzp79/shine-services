use axum::{extract::State, Extension, Json};
use serde::Serialize;
use shine_core::web::{
    permissions, CheckedCurrentUser, CorePermissions, IntoProblemResponse, ProblemConfig, ProblemResponse,
};
use utoipa::ToSchema;

use crate::app_state::AppState;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {}

#[utoipa::path(
    get,
    path = "/api/info/status",
    tag = "health",
    responses(
        (status = OK, body = ServiceStatus)
    )
)]
pub async fn get_service_status(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
) -> Result<Json<ServiceStatus>, ProblemResponse> {
    user.core_permissions()
        .check(permissions::READ_TRACE)
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(Json(ServiceStatus {}))
}
