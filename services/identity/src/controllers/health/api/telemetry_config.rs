use crate::{
    controllers::{ApiKind, AppState},
    services::Permission,
};
use axum::{extract::State, http::StatusCode, Extension, Json};
use serde::{Deserialize, Serialize};
use shine_core::{
    axum::{telemetry::DynConfig, ApiEndpoint, ApiMethod, Problem, ProblemConfig},
    service::CheckedCurrentUser,
};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TraceConfig {
    filter: String,
}

async fn put_telemetry_config(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    Json(body): Json<TraceConfig>,
) -> Result<(), Problem> {
    state.check_permission(&user, Permission::UpdateTrace).await?;

    log::trace!("reconfigure telemetry: {:#?}", body);
    state
        .telemetry_service()
        .set_configuration(DynConfig { filter: body.filter })
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to update configuration", err))?;

    Ok(())
}

pub fn ep_put_telemetry_config() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Put, ApiKind::Api("/telemetry/config"), put_telemetry_config)
        .with_operation_id("put_telemetry_config")
        .with_tag("health")
        .with_json_request::<TraceConfig>()
        .with_status_response(StatusCode::OK, "Telemetry configuration is updated")
}

async fn get_telemetry_config(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
) -> Result<Json<TraceConfig>, Problem> {
    state.check_permission(&user, Permission::UpdateTrace).await?;

    let config = state
        .telemetry_service()
        .get_configuration()
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to update configuration", err))?;

    Ok(Json(TraceConfig { filter: config.filter }))
}

pub fn ep_get_telemetry_config() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/telemetry/config"), get_telemetry_config)
        .with_operation_id("get_telemetry_config")
        .with_tag("health")
        .with_json_response::<TraceConfig>(StatusCode::OK)
}
