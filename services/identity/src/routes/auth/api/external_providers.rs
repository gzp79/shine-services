use axum::{extract::State, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::app_state::AppState;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthProviders {
    providers: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/api/auth/providers",
    tag = "auth",
    responses(
        (status = OK, body = AuthProviders)
    )
)]
pub async fn list_external_providers(State(state): State<AppState>) -> Json<AuthProviders> {
    let providers = state.settings().external_providers.clone();
    Json(AuthProviders { providers })
}
