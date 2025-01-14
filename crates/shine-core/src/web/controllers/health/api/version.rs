use axum::{Extension, Json};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, ToSchema, Serialize)]
pub struct ServiceVersion {
    pub app_name: String,
    pub version: String,
}

#[utoipa::path(
    get,
    path = "/info/version",
    tag = "health",
    description = "Get the version of the service.",
    responses(
        (status = OK, body = ServiceVersion)
    )
)]

pub async fn get_version(Extension(version): Extension<ServiceVersion>) -> Json<ServiceVersion> {
    Json(version)
}
