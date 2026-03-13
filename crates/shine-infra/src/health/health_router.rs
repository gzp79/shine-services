use axum::{Extension, Json};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Debug, Clone, ToSchema, Serialize)]
pub struct ServiceVersion {
    pub app_name: String,
    pub version: String,
}

#[utoipa::path(
    get,
    path = "/info/ready",
    tag = "health",
    description = "Health check.",
    responses(
        (status = OK, description = "Healthy.")
    )
)]
pub async fn get_ready() -> String {
    "Ok".into()
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

pub(super) fn build_router<S>(version: ServiceVersion) -> OpenApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    OpenApiRouter::new()
        .routes(routes!(get_ready))
        .routes(routes!(get_version))
        .layer(Extension(version))
}
