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
}

/// The application configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub db: DBConfig,
    pub ws: WsConfig,
    /// Hub heartbeat interval in seconds; Redis TTL is derived as heartbeat * 2.
    #[serde(default = "AppConfig::default_hub_heartbeat_seconds")]
    pub hub_heartbeat_seconds: u64,
    /// Interval in seconds between session-liveness re-checks.
    #[serde(default = "AppConfig::default_auth_check_interval")]
    pub auth_check_interval: u64,
}

impl AppConfig {
    fn default_hub_heartbeat_seconds() -> u64 {
        5 * 60
    }

    fn default_auth_check_interval() -> u64 {
        60
    }
}

impl FeatureConfig for AppConfig {
    const NAME: &'static str = "builder";
}
