use azure_identity::AzureCliCredential;
use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use shine_service::axum::tracing::TracingConfig;
use shine_service::azure::azure_keyvault_config::AzureKeyvaultConfigSource;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error as ThisError;
use tokio::runtime::Handle as RtHandle;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Slot {
    Test,
    Dev,
    Live,
}

/// Partial configuration required for early setup. These parameters shall not be altered
/// in the other layers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CoreConfig {
    pub slot: Slot,
    pub stage: String,

    pub shared_keyvault: Option<String>,
    pub private_keyvault: Option<String>,
}

impl CoreConfig {
    fn new(config_file: &str) -> Result<CoreConfig, ConfigError> {
        let builder = Config::builder()
            .add_source(Environment::default().separator("--"))
            .add_source(File::from(Path::new(config_file)));

        let s = builder.build()?;
        let cfg: CoreConfig = s.try_deserialize()?;

        log::info!("pre-init configuration: {:#?}", cfg);
        Ok(cfg)
    }
}

pub const SERVICE_NAME: &str = "shine-identity";
pub const DEFAULT_CONFIG_FILE: &str = "server_config.json";

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
pub struct AppConfig {
    #[serde(flatten)]
    pub core: CoreConfig,
    pub tracing: TracingConfig,
    pub control_port: u16,
}

impl AppConfig {
    pub fn new(rt_handle: &RtHandle) -> Result<AppConfig, ConfigError> {
        let pre_init = CoreConfig::new(DEFAULT_CONFIG_FILE)?;

        let mut builder = Config::builder();

        {
            log::warn!("Finding azure credentials...");
            let azure_credentials = Arc::new(AzureCliCredential::new());

            log::info!("Checking shared keyvault...");
            let shared_keyvault = pre_init
                .shared_keyvault
                .as_ref()
                .map(|uri| AzureKeyvaultConfigSource::new(rt_handle, azure_credentials.clone(), uri))
                .transpose()?;
            if let Some(shared_keyvault) = shared_keyvault {
                builder = builder.add_source(shared_keyvault)
            }

            log::info!("Checking private keyvault...");
            let private_keyvault = pre_init
                .private_keyvault
                .as_ref()
                .map(|uri| AzureKeyvaultConfigSource::new(rt_handle, azure_credentials.clone(), uri))
                .transpose()?;
            if let Some(private_keyvault) = private_keyvault {
                builder = builder.add_source(private_keyvault)
            }
        }

        builder = builder
            .add_source(File::from(Path::new(DEFAULT_CONFIG_FILE)))
            .add_source(Environment::default().separator("--"));

        let s = builder.build()?;
        let cfg: AppConfig = s.try_deserialize()?;

        if pre_init != cfg.core {
            return Err(PreInitConfigError.into());
        }

        log::info!("configuration: {:#?}", cfg);
        Ok(cfg)
    }
}
