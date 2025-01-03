mod api;

use crate::app_state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

pub struct HealthController();

impl HealthController {
    pub fn new() -> Self {
        Self()
    }

    pub fn into_router(self) -> OpenApiRouter<AppState> {
        OpenApiRouter::new().routes(routes!(api::get_service_status))
    }
}
