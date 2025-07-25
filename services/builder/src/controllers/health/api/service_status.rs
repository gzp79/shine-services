use axum::{Extension, Json};
use serde::Serialize;
use shine_infra::web::{
    responses::{IntoProblemResponse, ProblemConfig, ProblemResponse},
    session::{permissions, CheckedCurrentUser, CorePermissions},
};
use utoipa::ToSchema;

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
    //State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
) -> Result<Json<ServiceStatus>, ProblemResponse> {
    user.core_permissions()
        .check(permissions::READ_TRACE)
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(Json(ServiceStatus {}))
}
