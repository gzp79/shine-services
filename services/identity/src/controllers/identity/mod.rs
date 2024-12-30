mod api;

use crate::app_state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

pub struct IdentityController();

impl IdentityController {
    pub fn new() -> Self {
        Self()
    }

    pub fn into_router(self) -> OpenApiRouter<AppState> {
        OpenApiRouter::new()
            .routes(routes!(api::generate_user_name))
            .routes(routes!(api::search_identity))
            .routes(routes!(api::add_user_role))
            .routes(routes!(api::get_user_roles))
            .routes(routes!(api::delete_user_role))
    }
}
