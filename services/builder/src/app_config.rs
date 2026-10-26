use crate::repositories::DBConfig;
use serde::{Deserialize, Serialize};
use shine_infra::{serde::*, web::FeatureConfig};
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
    /// Hub heartbeat interval in seconds; Redis TTL is derived from  heartbeat timeout.
    #[serde(default = "u64::<300>")]
    #[validate(range(min = 1, max = 86_400))]
    pub hub_heartbeat_seconds: u64,
    /// Interval in seconds between session-liveness re-checks.
    #[serde(default = "u64::<60>")]
    pub auth_check_interval: u64,
}

impl FeatureConfig for AppConfig {
    const NAME: &'static str = "builder";

    fn validate(&self) -> Result<(), anyhow::Error> {
        Validate::validate(self).map_err(anyhow::Error::from)
    }
}
