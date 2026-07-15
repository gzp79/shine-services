use super::{generate_user_name, purge_guests, search_identity, user_roles};
use crate::app_state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

pub fn api_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(generate_user_name::generate_user_name))
        .routes(routes!(search_identity::search_identity))
        .routes(routes!(user_roles::add_user_role))
        .routes(routes!(user_roles::get_user_roles))
        .routes(routes!(user_roles::delete_user_role))
        .routes(routes!(purge_guests::purge_guests))
}
