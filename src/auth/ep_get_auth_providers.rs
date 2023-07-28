use crate::{auth::AuthServiceState, openapi::ApiKind};
use axum::{body::HttpBody, extract::State, http::StatusCode, Json};
use serde::Serialize;
use shine_service::axum::{ApiEndpoint, ApiMethod};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct AuthProviders {
    providers: Vec<String>,
}

async fn get_auth_providers(State(state): State<AuthServiceState>) -> Json<AuthProviders> {
    let providers = state.providers().to_vec();
    Json(AuthProviders { providers })
}

pub fn ep_get_auth_providers<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/providers"), get_auth_providers)
        .with_operation_id("ep_get_auth_providers")
        .with_tag("auth")
        .with_json_response::<AuthProviders>(StatusCode::OK)
}
