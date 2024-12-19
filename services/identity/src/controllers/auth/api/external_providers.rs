use crate::controllers::{ApiKind, AppState};
use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use shine_service::axum::{ApiEndpoint, ApiMethod};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct AuthProviders {
    providers: Vec<String>,
}

async fn list_external_providers(State(state): State<AppState>) -> Json<AuthProviders> {
    let providers = state.settings().external_providers.clone();
    Json(AuthProviders { providers })
}

pub fn ep_list_external_providers() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/providers"), list_external_providers)
        .with_operation_id("list_external_providers")
        .with_tag("auth")
        .with_json_response::<AuthProviders>(StatusCode::OK)
}
