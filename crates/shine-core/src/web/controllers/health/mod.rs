mod api;

use crate::web::WebAppConfig;
use anyhow::Error as AnyError;
use axum::Extension;
use utoipa_axum::{router::OpenApiRouter, routes};

pub struct HealthController {
    version: api::ServiceVersion,
}

impl HealthController {
    pub fn new<F>(feature_name: &'static str, config: &WebAppConfig<F>) -> Result<Self, AnyError> {
        let version = api::ServiceVersion {
            app_name: feature_name.to_string(),
            version: config.core.version.clone(),
        };
        Ok(Self { version })
    }

    pub fn into_routes<S>(self) -> OpenApiRouter<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let api = OpenApiRouter::new()
            .routes(routes!(api::get_ready))
            .routes(routes!(api::get_metrics))
            .routes(routes!(api::get_telemetry_config))
            .routes(routes!(api::put_telemetry_config));

        let version_api = {
            OpenApiRouter::new()
                .routes(routes!(api::get_version))
                .layer(Extension(self.version))
        };

        api.merge(version_api)
    }
}
