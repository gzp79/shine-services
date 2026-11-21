use crate::{
    repositories::DBConfig,
    routes::ws::WsConfig,
    services::{HubConfig, StreamsConfig},
};
use serde::{Deserialize, Serialize};
use shine_infra::web::{FeatureConfig, WebAppConfig};
use validator::Validate;

/// The application configuration
#[derive(Debug, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct BuilderConfig {
    pub db: DBConfig,
    pub ws: WsConfig,
    #[serde(default = "HubConfig::defaults")]
    pub hub: HubConfig,
    #[serde(default)]
    #[validate(nested)]
    pub streams: StreamsConfig,
}

impl FeatureConfig for BuilderConfig {
    const NAME: &'static str = "builder";

    fn validate(&self) -> Result<(), anyhow::Error> {
        Validate::validate(self).map_err(anyhow::Error::from)
    }
}

pub type AppConfig = WebAppConfig<BuilderConfig>;

pub trait AppConfigEx {
    fn ws_config(&self) -> &WsConfig;

    fn is_allowed_host(&self, host: &str) -> bool;
    fn is_allowed_origin(&self, origin: &str) -> bool;
}

impl AppConfigEx for WebAppConfig<BuilderConfig> {
    fn ws_config(&self) -> &WsConfig {
        &self.feature().ws
    }

    fn is_allowed_host(&self, host: &str) -> bool {
        self.ws_config().is_allowed_host(host)
    }

    fn is_allowed_origin(&self, origin: &str) -> bool {
        self.service().is_allowed_origin(origin)
    }
}
