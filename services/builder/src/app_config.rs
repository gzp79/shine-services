use crate::{repositories::DBConfig, services::HubConfig};
use serde::{Deserialize, Serialize};
use shine_infra::web::FeatureConfig;
use validator::Validate;

/// Raw WebSocket configuration loaded from config files.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsConfig {
    /// Regular expressions for allowed WebSocket target hosts.
    /// Checked against `X-Forwarded-Host`, `Forwarded host=`, then `Host`.
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
}

/// The application configuration
#[derive(Debug, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub db: DBConfig,
    pub ws: WsConfig,
    #[serde(default = "HubConfig::defaults")]
    pub hub: HubConfig,
}

impl FeatureConfig for AppConfig {
    const NAME: &'static str = "builder";

    fn validate(&self) -> Result<(), anyhow::Error> {
        Validate::validate(self).map_err(anyhow::Error::from)
    }
}
