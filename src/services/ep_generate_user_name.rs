use crate::services::IdentityServiceState;
use axum::{extract::State, Json};
use serde::Serialize;
use shine_service::axum::Problem;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    name: String,
}

pub(in crate::services) async fn ep_generate_user_name(
    State(state): State<IdentityServiceState>,
) -> Result<Json<Response>, Problem> {
    let name = state
        .name_generator()
        .generate_name()
        .await
        .map_err(Problem::internal_error_from)?;

    Ok(Json(Response { name }))
}
