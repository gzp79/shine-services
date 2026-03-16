use crate::{
    health::StatusProviders,
    session::{permissions, CheckedCurrentUser, CorePermissions},
    web::responses::{IntoProblemResponse, ProblemConfig, ProblemResponse},
};
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

#[utoipa::path(
    get,
    path = "/info/status",
    tag = "health",
    description = "Get the operational status of the service.",
    responses(
        (status = OK, description = "Service status")
    )
)]
pub async fn get_status(
    Extension(problem_config): Extension<ProblemConfig>,
    Extension(providers): Extension<StatusProviders>,
    user: CheckedCurrentUser,
) -> Result<Json<serde_json::Value>, ProblemResponse> {
    user.core_permissions()
        .check(permissions::READ_TRACE)
        .map_err(|err| err.into_response(&problem_config))?;

    let providers = providers.read().unwrap().clone();
    let mut status = serde_json::Map::new();
    for provider in providers.iter() {
        status.insert(provider.name().to_string(), provider.status().await);
    }

    Ok(Json(serde_json::Value::Object(status)))
}

pub(super) fn build_router<S>(version: ServiceVersion, providers: StatusProviders) -> OpenApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    OpenApiRouter::new()
        .routes(routes!(get_ready))
        .routes(routes!(get_version))
        .routes(routes!(get_status))
        .layer(Extension(version))
        .layer(Extension(providers))
}
