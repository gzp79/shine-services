use crate::{
    service::CheckedCurrentUser,
    telemetry::{DynConfig, TelemetryService},
    web::{Problem, ProblemConfig},
};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TraceConfig {
    filter: String,
}

#[utoipa::path(
    put,
    path = "/api/telemetry/config", 
    tag = "health",
    description = "Update telemetry configuration.",
    request_body = TraceConfig,
    responses(
        (status = OK, description = "Telemetry configuration is updated.")
    )
)]
pub async fn put_telemetry_config(
    Extension(telemetry): Extension<TelemetryService>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    Json(body): Json<TraceConfig>,
) -> Result<(), Problem> {
    //fixme: state.check_permission(&user, Permission::UpdateTrace).await?;

    log::trace!("reconfigure telemetry: {:#?}", body);
    telemetry
        .set_configuration(DynConfig { filter: body.filter })
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to update configuration", err))?;

    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/telemetry/config", 
    tag = "health",
    description = "Get the current telemetry configuration.",
    responses(
        (status = OK, body = TraceConfig)
    )
)]

pub async fn get_telemetry_config(
    Extension(telemetry): Extension<TelemetryService>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
) -> Result<Json<TraceConfig>, Problem> {
    //fixme: state.check_permission(&user, Permission::UpdateTrace).await?;

    let config = telemetry
        .get_configuration()
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to update configuration", err))?;

    Ok(Json(TraceConfig { filter: config.filter }))
}
