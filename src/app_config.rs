use crate::{auth, db::DBConfig};
use config::ConfigError;
use serde::{Deserialize, Serialize};
use shine_service::axum::tracing::TracingConfig;
use shine_service::service::CoreConfig;
use thiserror::Error as ThisError;
use url::Url;

pub const SERVICE_NAME: &str = "identity";

#[derive(Debug, ThisError)]
#[error("Pre-init configuration is not matching to the final configuration")]
pub struct PreInitConfigError;

impl From<PreInitConfigError> for ConfigError {
    fn from(err: PreInitConfigError) -> Self {
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
pub struct AppConfig {
    #[serde(flatten)]
    pub core: CoreConfig,

    pub tracing: TracingConfig,
    pub auth: auth::AuthConfig,
    pub db: DBConfig,
    pub control_port: u16,

    pub allow_origins: Vec<String>,
    pub cookie_secret: String,
    pub home_url: Url,
    pub tls: Option<TlsConfig>,

    pub session_max_duration: usize,
}

impl AppConfig {
    pub async fn new() -> Result<AppConfig, ConfigError> {
        let pre_init = CoreConfig::new()?;
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
