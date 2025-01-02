mod api;

use utoipa_axum::{router::OpenApiRouter, routes};

pub struct HealthController;

impl HealthController {
    pub fn into_routes<S>(self) -> OpenApiRouter<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        OpenApiRouter::new()
            .routes(routes!(api::get_ready))
            .routes(routes!(api::get_metrics))
            .routes(routes!(api::get_telemetry_config))
            .routes(routes!(api::put_telemetry_config))
    }
}
