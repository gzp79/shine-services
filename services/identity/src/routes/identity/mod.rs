mod api;

use crate::app_state::AppState;
use utoipa_axum::router::OpenApiRouter;

pub struct IdentityRouter();

impl IdentityRouter {
    pub fn new() -> Self {
        Self()
    }

    pub fn into_router(self) -> OpenApiRouter<AppState> {
        api::api_routes()
    }
}
