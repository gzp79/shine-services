use crate::{openapi::ApiKind, services::IdentityServiceState};
use axum::{body::HttpBody, extract::State, http::StatusCode, BoxError, Json};
use serde::Serialize;
use shine_service::axum::{ApiEndpoint, ApiMethod, Problem};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "name": "Guest_123"
}))]
pub struct GeneratedUserName {
    name: String,
}

async fn generate_user_name(State(state): State<IdentityServiceState>) -> Result<Json<GeneratedUserName>, Problem> {
    let name = state
        .name_generator()
        .generate_name()
        .await
        .map_err(Problem::internal_error_from)?;

    Ok(Json(GeneratedUserName { name }))
}

pub fn ep_generate_user_name<B>() -> ApiEndpoint<IdentityServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Post, ApiKind::Api("/user-name"), generate_user_name)
        .with_operation_id("ep_generate_user_name")
        .with_tag("identity")
        .with_json_response::<GeneratedUserName>(StatusCode::OK)
}
