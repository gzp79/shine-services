use crate::repositories::DBConfig;
use serde::{Deserialize, Serialize};
use shine_infra::web::FeatureConfig;

/// Raw WebSocket configuration loaded from config files.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsConfig {
    /// Regular expressions for allowed WebSocket target hosts.
    /// Checked against `X-Forwarded-Host`, `Forwarded host=`, then `Host`.
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    /// Interval in seconds between auth re-checks for active WebSocket connections.
    #[serde(default = "WsConfig::default_auth_check_interval")]
    pub auth_check_interval: u64,
}

impl WsConfig {
    fn default_auth_check_interval() -> u64 {
        60
    }
}

/// The application configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub db: DBConfig,
    pub ws: WsConfig,
}

impl FeatureConfig for AppConfig {
    const NAME: &'static str = "builder";
}
