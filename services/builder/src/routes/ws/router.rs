use crate::{app_state::AppState, routes::ws::connect};
use utoipa_axum::{router::OpenApiRouter, routes};

pub fn ws_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(connect::connect))
}
