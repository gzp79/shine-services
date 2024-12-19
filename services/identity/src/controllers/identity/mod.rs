mod api;

use super::AppState;
use axum::Router;
use shine_service::axum::ApiRoute;
use utoipa::openapi::OpenApi;

pub struct IdentityController();

impl IdentityController {
    pub fn new() -> Self {
        Self()
    }

    pub fn into_router(self, doc: &mut OpenApi) -> Router<AppState> {
        Router::new()
            .add_api(api::ep_generate_user_name(), doc)
            .add_api(api::ep_search_identity(), doc)
            .add_api(api::ep_add_user_role(), doc)
            .add_api(api::ep_get_user_roles(), doc)
            .add_api(api::ep_delete_user_role(), doc)
    }
}
