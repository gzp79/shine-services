use crate::app_config::CommandStreamConfig;
use anyhow::{anyhow, Error as AnyError};
use std::{collections::HashMap, time::Duration};

/// Snapshot retention of one stream kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindSettings {
    /// A snapshot is stored every this many versions.
    pub snapshot_every: i64,
    /// Snapshots kept per stream; the commands are kept from the oldest of them on.
    pub keep_snapshots: i64,
}

impl KindSettings {
    /// Whether a snapshot is taken (and a checkpoint pushed) at the version.
    pub fn is_snapshot_version(&self, version: i64) -> bool {
        version > 0 && version % self.snapshot_every == 0
    }
}

/// Normalized command stream configuration.
#[derive(Clone, Debug)]
pub struct CommandStreamSettings {
    defaults: KindSettings,
    kinds: HashMap<String, KindSettings>,
    /// Unpersisted commands per stream before writes are refused.
    pub redis_max_unpersisted: i64,
    /// Inactivity TTL of the Redis keys of a stream.
    pub redis_ttl_seconds: i64,
    /// Interval of the periodic sync of the stream workers.
    pub sync_interval: Duration,
    /// Delay before new commands are persisted; each instance adds up to the same again as jitter.
    pub persist_delay: Duration,
    /// Time a stream stays loaded without local subscribers.
    pub unload_grace: Duration,
    /// Interval of the orphan sweep.
    pub orphan_sweep_interval: Duration,
    /// Time without writes before a stream is swept.
    pub orphan_idle: Duration,
    /// Max size of a command's JSON.
    pub max_command_bytes: usize,
}

fn kind_settings(name: &str, snapshot_every: i64, keep_snapshots: i64) -> Result<KindSettings, AnyError> {
    if snapshot_every < 1 {
        return Err(anyhow!("commandStream {name}: snapshotEvery must be at least 1"));
    }
    if keep_snapshots < 1 {
        return Err(anyhow!("commandStream {name}: keepSnapshots must be at least 1"));
    }
    Ok(KindSettings { snapshot_every, keep_snapshots })
}

impl CommandStreamSettings {
    /// Validates the configuration; an invalid retention fails the start.
    pub fn from_config(config: &CommandStreamConfig) -> Result<Self, AnyError> {
        let defaults = kind_settings("defaults", config.snapshot_every, config.keep_snapshots)?;
        let kinds = config
            .kinds
            .iter()
            .map(|(name, kind)| {
                let settings = kind_settings(
                    name,
                    kind.snapshot_every.unwrap_or(defaults.snapshot_every),
                    kind.keep_snapshots.unwrap_or(defaults.keep_snapshots),
                )?;
                Ok((name.clone(), settings))
            })
            .collect::<Result<HashMap<_, _>, AnyError>>()?;

        Ok(Self {
            defaults,
            kinds,
            redis_max_unpersisted: config.redis_max_unpersisted.max(1),
            redis_ttl_seconds: config.redis_ttl_seconds.max(60),
            sync_interval: Duration::from_millis(config.sync_interval_ms.max(10)),
            persist_delay: Duration::from_millis(config.persist_delay_ms.max(1)),
            unload_grace: Duration::from_secs(config.unload_grace_seconds),
            orphan_sweep_interval: Duration::from_secs(config.orphan_sweep_seconds.max(1)),
            orphan_idle: Duration::from_secs(config.orphan_idle_seconds),
            max_command_bytes: config.max_command_bytes.max(1),
        })
    }

    /// The retention of a kind: its own settings over the defaults.
    pub fn for_kind(&self, kind: &str) -> KindSettings {
        self.kinds.get(kind).copied().unwrap_or(self.defaults)
    }
}

#[cfg(test)]
#[path = "command_stream_test.rs"]
mod command_stream_test;
