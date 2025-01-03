mod api;

use crate::app_state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

pub struct BuilderController();

impl BuilderController {
    pub fn new() -> Self {
        Self()
    }

    pub fn into_router(self) -> OpenApiRouter<AppState> {
        OpenApiRouter::new().routes(routes!(api::connect))
    }
}
