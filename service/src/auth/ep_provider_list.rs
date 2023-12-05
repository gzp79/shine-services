use crate::{auth::AuthServiceState, openapi::ApiKind};
use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use shine_service::axum::{ApiEndpoint, ApiMethod};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct AuthProviders {
    providers: Vec<String>,
}

async fn provider_list(State(state): State<AuthServiceState>) -> Json<AuthProviders> {
    let providers = state.providers().to_vec();
    Json(AuthProviders { providers })
}

pub fn ep_provider_list() -> ApiEndpoint<AuthServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/providers"), provider_list)
        .with_operation_id("provider_list")
        .with_tag("auth")
        .with_json_response::<AuthProviders>(StatusCode::OK)
}
