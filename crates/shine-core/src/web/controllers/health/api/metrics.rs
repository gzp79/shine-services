use crate::telemetry::TelemetryService;
use axum::Extension;

#[utoipa::path(
    get, 
    path = "/api/telemetry/metrics", 
    tag = "health",
    description = "Get system metrics.",
    responses(
        (status = OK, description = "System metrics.")
    )
)]
pub async fn get_metrics(Extension(telemetry): Extension<TelemetryService>) -> String {
    telemetry.metrics()
}
