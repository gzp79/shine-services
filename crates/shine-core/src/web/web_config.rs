use crate::telemetry::TelemetryConfig;
use anyhow::{anyhow, Error as AnyError};
use serde::{de::DeserializeOwned, Deserialize};
use std::fmt::Debug;

use super::{CoreConfig, ServiceConfig};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAppConfig<F> {
    #[serde(flatten)]
    pub core: CoreConfig,
    pub service: ServiceConfig,
    pub telemetry: TelemetryConfig,

    #[serde(flatten)]
    pub feature: F,
}

impl<F> WebAppConfig<F>
where
    F: DeserializeOwned + Debug,
{
    pub async fn load_config(stage: &str) -> Result<Self, AnyError> {
        let pre_init = CoreConfig::new(stage)?;
        let builder = pre_init.create_config_builder()?;
        let config = builder.build().await?;

        let cfg: Self = config.try_deserialize()?;
        log::info!("Config loaded: {:#?}", cfg);

        if pre_init != cfg.core {
            Err(anyhow!("Core config mismatch"))
        } else {
            Ok(cfg)
        }
    }
}
