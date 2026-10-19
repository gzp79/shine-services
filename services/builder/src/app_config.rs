use crate::repositories::DBConfig;
use serde::{Deserialize, Serialize};
use shine_infra::web::FeatureConfig;
use std::collections::HashMap;

/// Raw WebSocket configuration loaded from config files.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsConfig {
    /// Regular expressions for allowed WebSocket target hosts.
    /// Checked against `X-Forwarded-Host`, `Forwarded host=`, then `Host`.
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
}

/// Snapshot retention of one stream kind; unset keys keep the defaults of `CommandStreamConfig`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KindRetentionConfig {
    pub snapshot_every: Option<i64>,
    pub keep_snapshots: Option<i64>,
    /// Keep every command and snapshot in Postgres: nothing of the stream is ever pruned.
    pub keep_history: bool,
}

/// Raw command stream configuration loaded from config files.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CommandStreamConfig {
    /// The streams are off until enabled; enabling needs `db.sqlCns`.
    pub enabled: bool,
    /// Default snapshot cadence, in versions.
    pub snapshot_every: i64,
    /// Default number of snapshots kept per stream.
    pub keep_snapshots: i64,
    /// Retention overrides by stream kind.
    pub kinds: HashMap<String, KindRetentionConfig>,
    /// Unpersisted commands per stream before writes are refused.
    pub redis_max_unpersisted: i64,
    /// Inactivity TTL of the Redis keys of a stream.
    pub redis_ttl_seconds: i64,
    /// Interval of the periodic sync of the stream workers.
    pub sync_interval_ms: u64,
    /// Delay before new commands are persisted; each instance adds up to the same again as jitter.
    pub persist_delay_ms: u64,
    /// Time a stream stays loaded without local subscribers.
    pub unload_grace_seconds: u64,
    /// Interval of the orphan sweep.
    pub orphan_sweep_seconds: u64,
    /// Time without writes before a stream is swept.
    pub orphan_idle_seconds: u64,
    /// Max size of a command's JSON.
    pub max_command_bytes: usize,
}

impl Default for CommandStreamConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            snapshot_every: 100,
            keep_snapshots: 3,
            kinds: HashMap::new(),
            redis_max_unpersisted: 5000,
            redis_ttl_seconds: 24 * 60 * 60,
            sync_interval_ms: 1000,
            persist_delay_ms: 20,
            unload_grace_seconds: 60,
            orphan_sweep_seconds: 30,
            orphan_idle_seconds: 10,
            max_command_bytes: 16 * 1024,
        }
    }
}

/// The application configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub db: DBConfig,
    pub ws: WsConfig,
    #[serde(default)]
    pub command_stream: CommandStreamConfig,
    /// Hub heartbeat interval in seconds; Redis TTL is derived from  heartbeat timeout.
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
