mod search_identity;
use self::search_identity::*;
mod generate_user_name;
use self::generate_user_name::*;
mod user_roles;
use self::user_roles::*;

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
            .add_api(ep_generate_user_name(), doc)
            .add_api(ep_search_identity(), doc)
            .add_api(ep_add_user_role(), doc)
            .add_api(ep_get_user_roles(), doc)
            .add_api(ep_delete_user_role(), doc)
    }
}
