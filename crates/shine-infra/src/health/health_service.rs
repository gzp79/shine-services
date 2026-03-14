use crate::web::{FeatureConfig, WebAppConfig};
use anyhow::Error as AnyError;
use axum::Extension;
use utoipa_axum::router::OpenApiRouter;

use super::health_router::ServiceVersion;

pub struct HealthService {
    version: ServiceVersion,
}

impl HealthService {
    pub fn new<F>(feature_name: &'static str, config: &WebAppConfig<F>) -> Result<Self, AnyError>
    where
        F: FeatureConfig,
    {
        Ok(Self {
            version: ServiceVersion {
                app_name: feature_name.to_string(),
                version: config.core.version.clone(),
            },
        })
    }

    pub fn create_layer(&self) -> Extension<ServiceVersion> {
        Extension(self.version.clone())
    }

    pub fn create_router<S>(&self) -> OpenApiRouter<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        super::health_router::build_router(self.version.clone())
    }
}
