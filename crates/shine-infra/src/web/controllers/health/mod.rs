mod api;

use crate::web::{FeatureConfig, WebAppConfig};
use anyhow::Error as AnyError;
use axum::Extension;
use utoipa_axum::{router::OpenApiRouter, routes};

pub struct HealthRouter {
    version: api::ServiceVersion,
}

impl HealthRouter {
    pub fn new<F>(feature_name: &'static str, config: &WebAppConfig<F>) -> Result<Self, AnyError>
    where
        F: FeatureConfig,
    {
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
        let api = OpenApiRouter::new().routes(routes!(api::get_ready));

        let version_api = {
            OpenApiRouter::new()
                .routes(routes!(api::get_version))
                .layer(Extension(self.version))
        };

        api.merge(version_api)
    }
}
