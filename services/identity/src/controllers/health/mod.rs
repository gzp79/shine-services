mod api;

use super::AppState;
use axum::Router;
use shine_core::axum::ApiRoute;
use utoipa::openapi::OpenApi;

pub struct HealthController();

impl HealthController {
    pub fn new() -> Self {
        Self()
    }

    pub fn into_router(self, doc: &mut OpenApi) -> Router<AppState> {
        Router::new()
            .add_api(api::ep_get_ready(), doc)
            .add_api(api::ep_get_service_status(), doc)
            .add_api(api::ep_get_metrics(), doc)
            .add_api(api::ep_get_telemetry_config(), doc)
            .add_api(api::ep_put_telemetry_config(), doc)
    }
}
