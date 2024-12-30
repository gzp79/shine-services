use crate::app_state::AppState;
use axum::{extract::State, Extension, Json};
use serde::Serialize;
use shine_core::web::{Problem, ProblemConfig};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "name": "Guest_123"
}))]
pub struct GeneratedUserName {
    name: String,
}

#[utoipa::path(
    post,
    path = "/api/user-name",
    tag = "identity",
    responses(
        (status = OK, body = GeneratedUserName)
    )
)]
pub async fn generate_user_name(
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
