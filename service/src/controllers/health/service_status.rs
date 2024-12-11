use crate::controllers::{ApiKind, AppState};
use axum::{extract::State, http::StatusCode, Json};
use bb8::State as BB8PoolState;
use serde::Serialize;
use shine_service::axum::{ApiEndpoint, ApiMethod};
use utoipa::ToSchema;

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
struct ServiceHealth {
    pub postgres: DBState,
    pub redis: DBState,
}

async fn get_service_status(State(state): State<AppState>) -> Json<ServiceHealth> {
    Json(ServiceHealth {
        postgres: DBState::from(state.db().postgres.state()),
        redis: DBState::from(state.db().redis.state()),
    })
}

pub fn ep_get_service_status() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/telemetry/status"), get_service_status)
        .with_operation_id("get_service_status")
        .with_tag("health")
        .with_schema::<DBState>()
        .with_json_response::<ServiceHealth>(StatusCode::OK)
}
