use crate::{
    auth,
    repositories::{AutoNameConfig, DBConfig},
};
use config::ConfigError;
use serde::{Deserialize, Serialize};
use shine_service::axum::telemetry::TelemetryConfig;
use shine_service::service::CoreConfig;
use thiserror::Error as ThisError;
use tower_http::cors::AllowOrigin;

pub const SERVICE_NAME: &str = "identity";

#[derive(Debug, ThisError)]
#[error("Pre-init configuration is not matching to the final configuration")]
pub struct PreInitConfigError;

impl From<PreInitConfigError> for ConfigError {
    fn from(err: PreInitConfigError) -> Self {
        ConfigError::Foreign(Box::new(err))
    }
}

#[derive(Debug, ThisError)]
#[error("CORS configuration is not a valid")]
pub struct CORSConfigError;

impl From<CORSConfigError> for ConfigError {
    fn from(err: CORSConfigError) -> Self {
        ConfigError::Foreign(Box::new(err))
    }
}

/// The application configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsConfig {
    pub cert: String,
    pub key: String,
}

/// The application configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfig {
    pub tls: Option<TlsConfig>,
    pub port: u16,
    /// Regular expression for the allowed origins.
    pub allowed_origins: Vec<String>,
}

impl ServiceConfig {
    pub fn cors_allowed_origin(&self) -> Result<AllowOrigin, CORSConfigError> {
        let allowed_origins = self
            .allowed_origins
            .iter()
            .map(|r| regex::bytes::Regex::new(r))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_err| CORSConfigError)?;
        Ok(AllowOrigin::predicate(move |origin, _| {
            let origin = origin.as_bytes();
            allowed_origins.iter().any(|r| r.is_match(origin))
        }))
    }
}

/// The application configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(flatten)]
    pub core: CoreConfig,

    pub service: ServiceConfig,
    pub telemetry: TelemetryConfig,
    pub db: DBConfig,
    pub auth: auth::AuthConfig,
    pub user_name: AutoNameConfig,
}

impl AppConfig {
    pub async fn new(stage: &str) -> Result<AppConfig, ConfigError> {
        let pre_init = CoreConfig::new(stage)?;
        let builder = pre_init.create_config_builder()?;
        let config = builder.build().await?;
        log::debug!("configuration values: {:#?}", config);

        let cfg: AppConfig = config.try_deserialize()?;
        if pre_init != cfg.core {
            return Err(PreInitConfigError.into());
        }

        log::info!("configuration: {:#?}", cfg);
        Ok(cfg)
    }
}
