use crate::services::IdentityServiceState;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Json,
};
use reqwest::StatusCode;
use serde::Serialize;

#[derive(Serialize)]
pub struct UserName {
    name: String,
}

pub(in crate::services) async fn get_username(
    State(state): State<IdentityServiceState>,
) -> Result<Json<UserName>, Response> {
    let name = state
        .name_generator()
        .generate_name()
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:?}")).into_response())?;

    Ok(Json(UserName { name }))
}
