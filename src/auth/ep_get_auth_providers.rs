use crate::{auth::AuthServiceState, openapi::ApiKind};
use axum::{body::HttpBody, extract::State, Json};
use shine_service::axum::{ApiEndpoint, ApiMethod};

async fn get_auth_providers(State(state): State<AuthServiceState>) -> Json<Vec<String>> {
    let providers = state.providers().to_vec();
    Json(providers)
}

pub fn ep_get_auth_providers<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/providers"), get_auth_providers)
        .with_operation_id("ep_get_auth_providers")
        .with_tag("auth")
}
