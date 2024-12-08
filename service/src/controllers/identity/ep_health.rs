use crate::{identity::IdentityServiceState, openapi::ApiKind, repositories::Permission};
use axum::{extract::State, http::StatusCode, Extension, Json};
use bb8::State as BB8PoolState;
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, IntoProblem, Problem, ProblemConfig},
    service::CheckedCurrentUser,
};
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

async fn health(State(state): State<IdentityServiceState>) -> Json<ServiceHealth> {
    Json(ServiceHealth {
        postgres: DBState::from(state.db().postgres.state()),
        redis: DBState::from(state.db().redis.state()),
    })
}

pub fn ep_health() -> ApiEndpoint<IdentityServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/telemetry/health"), health)
        .with_operation_id("ep_health")
        .with_tag("status")
        .with_schema::<DBState>()
        .with_json_response::<ServiceHealth>(StatusCode::OK)
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(as=UpdateTraceConfig)]
pub struct Request {
    filter: String,
}

async fn reconfigure_telemetry(
    State(state): State<IdentityServiceState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    Json(format): Json<Request>,
) -> Result<(), Problem> {
    state
        .require_permission(&user, Permission::UpdateTrace)
        .await
        .map_err(|err| err.into_problem(&problem_config))?;

    log::trace!("config: {:#?}", format);
    state
        .telemetry_manager()
        .reconfigure(format.filter)
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to update configuration", err))?;

    Ok(())
}

pub fn ep_reconfigure_telemetry() -> ApiEndpoint<IdentityServiceState> {
    ApiEndpoint::new(ApiMethod::Put, ApiKind::Api("/telemetry/config"), reconfigure_telemetry)
        .with_operation_id("reconfigure_telemetry")
        .with_tag("status")
        .with_json_request::<Request>()
        .with_status_response(StatusCode::OK, "Telemetry configuration is updated")
}

async fn metrics(State(state): State<IdentityServiceState>) -> String {
    state.telemetry_manager().metrics()
}

pub fn ep_metrics() -> ApiEndpoint<IdentityServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/telemetry/metrics"), metrics)
        .with_operation_id("metrics")
        .with_tag("status")
        .with_status_response(StatusCode::OK, "Ok.")
}
