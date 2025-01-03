use axum::{extract::State, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::app_state::AppState;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHealth {}

#[utoipa::path(
    get,
    path = "/api/info/status",
    tag = "health",
    responses(
        (status = OK, body = ServiceHealth)
    )
)]
pub async fn get_service_status(State(state): State<AppState>) -> Json<ServiceHealth> {
    Json(ServiceHealth {})
}
