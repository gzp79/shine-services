use crate::controllers::{ApiKind, AppState};
use axum::{extract::State, http::StatusCode, Extension, Json};
use serde::Serialize;
use shine_core::axum::{ApiEndpoint, ApiMethod, Problem, ProblemConfig};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "name": "Guest_123"
}))]
pub struct GeneratedUserName {
    name: String,
}

async fn generate_user_name(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
) -> Result<Json<GeneratedUserName>, Problem> {
    let name = state
        .identity_service()
        .generate_user_name()
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to generate name", err))?;

    Ok(Json(GeneratedUserName { name }))
}

pub fn ep_generate_user_name() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Post, ApiKind::Api("/user-name"), generate_user_name)
        .with_operation_id("generate_user_name")
        .with_tag("identity")
        .with_json_response::<GeneratedUserName>(StatusCode::OK)
}
