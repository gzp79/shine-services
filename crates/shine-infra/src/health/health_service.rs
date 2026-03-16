use crate::{
    health::{health_router::build_router, ServiceVersion},
    web::{FeatureConfig, WebAppConfig},
};
use anyhow::Error as AnyError;
use async_trait::async_trait;
use axum::Extension;
use std::sync::{Arc, RwLock};
use utoipa_axum::router::OpenApiRouter;

#[async_trait]
pub trait StatusProvider: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    async fn status(&self) -> serde_json::Value;
}

pub type StatusProviders = Arc<RwLock<Vec<Arc<dyn StatusProvider>>>>;

pub struct HealthService {
    version: ServiceVersion,
    providers: StatusProviders,
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
            providers: Arc::new(RwLock::new(Vec::new())),
        })
    }

    pub fn add_provider<P: StatusProvider>(&mut self, provider: P) {
        self.providers.write().unwrap().push(Arc::new(provider));
    }

    pub fn create_layer(&self) -> Extension<ServiceVersion> {
        Extension(self.version.clone())
    }

    pub fn create_router<S>(&self) -> OpenApiRouter<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        build_router(self.version.clone(), self.providers.clone())
    }
}
