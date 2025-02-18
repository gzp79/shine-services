use crate::{app_state::AppState, services::permissions};
use axum::{extract::State, Extension, Json};
use bb8::State as BB8PoolState;
use serde::Serialize;
use shine_core::web::{CheckedCurrentUser, CorePermissions, IntoProblemResponse, ProblemConfig, ProblemResponse};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DBState {
    pub connections: u32,
    pub idle_connections: u32,
}

impl From<BB8PoolState> for DBState {
    fn from(value: BB8PoolState) -> Self {
        Self {
            connections: value.connections,
            idle_connections: value.idle_connections,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub postgres: DBState,
    pub redis: DBState,
}

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

    Ok(Json(ServiceStatus {
        postgres: DBState::from(state.db().postgres.state()),
        redis: DBState::from(state.db().redis.state()),
    }))
}
