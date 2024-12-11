mod ready;
use self::ready::*;
mod service_status;
use self::service_status::*;
mod telemetry_config;
use self::telemetry_config::*;
mod metrics;
use self::metrics::*;

use super::AppState;
use axum::Router;
use shine_service::axum::ApiRoute;
use utoipa::openapi::OpenApi;

pub struct HealthController();

impl HealthController {
    pub fn new() -> Self {
        Self()
    }

    pub fn into_router(self, doc: &mut OpenApi) -> Router<AppState> {
        Router::new()
            .add_api(ep_get_ready(), doc)
            .add_api(ep_get_service_status(), doc)
            .add_api(ep_get_metrics(), doc)
            .add_api(ep_get_telemetry_config(), doc)
            .add_api(ep_put_telemetry_config(), doc)
    }
}
