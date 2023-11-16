use crate::{auth::AuthServiceState, openapi::ApiKind};
use axum::{body::HttpBody, extract::State, http::StatusCode, Json};
use serde::Serialize;
use shine_service::axum::{ApiEndpoint, ApiMethod};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as=AuthProviders)]
struct Response {
    providers: Vec<String>,
}

async fn provider_list(State(state): State<AuthServiceState>) -> Json<Response> {
    let providers = state.providers().to_vec();
    Json(Response { providers })
}

pub fn ep_provider_list<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/providers"), provider_list)
        .with_operation_id("provider_list")
        .with_tag("auth")
        .with_json_response::<Response>(StatusCode::OK)
}
