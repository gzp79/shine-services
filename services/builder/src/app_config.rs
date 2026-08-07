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

/// Raw chat configuration loaded from config files.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatConfig {
    /// Approximate number of chat messages retained per room stream.
    #[serde(default = "ChatConfig::default_stream_max_messages")]
    pub stream_max_messages: usize,
    /// Room stream inactivity TTL in seconds.
    #[serde(default = "ChatConfig::default_stream_ttl_seconds")]
    pub stream_ttl_seconds: u64,
    /// Interval in seconds between chat dispatcher sync ticks.
    #[serde(default = "ChatConfig::default_sync_interval_seconds")]
    pub sync_interval_seconds: u64,
}

impl ChatConfig {
    fn default_stream_max_messages() -> usize {
        200
    }

    fn default_stream_ttl_seconds() -> u64 {
        60 * 60
    }

    fn default_sync_interval_seconds() -> u64 {
        1
    }
}

/// The application configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub db: DBConfig,
    pub ws: WsConfig,
    #[serde(default)]
    pub chat: ChatConfig,
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

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            stream_max_messages: Self::default_stream_max_messages(),
            stream_ttl_seconds: Self::default_stream_ttl_seconds(),
            sync_interval_seconds: Self::default_sync_interval_seconds(),
        }
    }
}

impl FeatureConfig for AppConfig {
    const NAME: &'static str = "builder";
}
