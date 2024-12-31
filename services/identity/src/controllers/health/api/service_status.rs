use axum::{extract::State, Json};
use bb8::State as BB8PoolState;
use serde::Serialize;
use utoipa::ToSchema;

use crate::app_state::AppState;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct DBState {
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
pub struct ServiceHealth {
    pub app: String,
    pub version: String,
    pub postgres: DBState,
    pub redis: DBState,
}

#[utoipa::path(
    get,
    path = "/api/telemetry/status",
    tag = "health",
    responses(
        (status = OK, body = ServiceHealth)
    )
)]
pub async fn get_service_status(State(state): State<AppState>) -> Json<ServiceHealth> {
    Json(ServiceHealth {
        app: state.settings().app_name.clone(),
        version: state.settings().app_version.clone(),
        postgres: DBState::from(state.db().postgres.state()),
        redis: DBState::from(state.db().redis.state()),
    })
}
