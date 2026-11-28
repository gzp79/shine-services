use serde::{Deserialize, Serialize};
use shine_core::{serde::default_from_serde, validation::non_zero_duration};
use shine_game::replica::{StreamId, StreamKind};
use shine_infra::serde::{consts, duration};
use std::time::Duration;
use validator::{Validate, ValidationError, ValidationErrors};

/// The settings of the Redis inbox of a stream.
#[derive(Clone, Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
#[validate(schema(function = "validate_inbox_config"))]
pub struct InboxConfig {
    /// Most inbox entries read in one sync.
    #[serde(default = "consts::usize::<128>")]
    #[validate(range(min = 1))]
    pub poll_limit: usize,
    /// Seconds between periodic fallback inbox checks. Notifications normally wake the worker;
    /// these occasional checks recover commands whose notifications were missed.
    #[serde(default = "duration::seconds::<10>", with = "duration::seconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub recovery_poll_interval: Duration,
    /// How many times a read of the inbox that fails because Redis is out of reach is tried again before the
    /// stream is given up; `0` gives up at once. The worker does nothing else meanwhile. With the default
    /// delays the retries last about three to five seconds, longer than a Redis restart or a failover
    /// hiccup, short enough not to keep a stream stalled on a Redis that is gone.
    #[serde(default = "consts::u32::<8>")]
    pub max_retries: u32,
    /// Milliseconds before the first retry of a failing inbox read. The waits double (and are jittered) up to
    /// `retry_max_delay`.
    #[serde(default = "duration::milliseconds::<100>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub retry_min_delay: Duration,
    /// Milliseconds the longest wait between the retries of a failing inbox read lasts.
    #[serde(default = "duration::milliseconds::<1000>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub retry_max_delay: Duration,
    /// Pushes wait once the inbox holds this many entries.
    #[serde(default = "consts::usize::<1024>")]
    pub backpressure_high: usize,
    /// Pushes wait until it is down to this many entries once the backpressure is triggered.
    #[serde(default = "consts::usize::<256>")]
    pub backpressure_low: usize,
    /// Milliseconds between the checks of the inbox length for the backpressure.
    #[serde(default = "duration::milliseconds::<250>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub backpressure_poll_period: Duration,
    /// Milliseconds a push waits for the backpressure before the connection is dropped as busy.
    #[serde(default = "duration::milliseconds::<2000>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub backpressure_timeout: Duration,
}

fn validate_inbox_config(config: &InboxConfig) -> Result<(), ValidationError> {
    if config.backpressure_low >= config.backpressure_high {
        return Err(ValidationError::new("watermarks")
            .with_message("backpressureLow must be less than backpressureHigh".into()));
    }
    if config.retry_min_delay > config.retry_max_delay {
        return Err(ValidationError::new("retryDelays")
            .with_message("retryMinDelay must not be greater than retryMaxDelay".into()));
    }
    Ok(())
}

impl Default for InboxConfig {
    fn default() -> Self {
        default_from_serde()
    }
}

/// The settings of the Postgres log of a stream.
#[derive(Clone, Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
#[validate(schema(function = "validate_log_config"))]
pub struct LogConfig {
    /// How many times a call to the log that fails because Postgres is out of reach is tried again before the
    /// stream is given up; `0` gives up at once. Only the calls that are safe to repeat are retried: the reads, and
    /// the append, which the log recognises as already stored if the first try got through.
    #[serde(default = "consts::u32::<8>")]
    pub max_retries: u32,
    /// Milliseconds before the first retry of a failing call to the log. The waits double (and are jittered) up
    /// to `retry_max_delay`.
    #[serde(default = "duration::milliseconds::<100>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub retry_min_delay: Duration,
    /// Milliseconds the longest wait between the retries of a failing call to the log lasts.
    #[serde(default = "duration::milliseconds::<1000>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub retry_max_delay: Duration,
    /// Most commands read from the log in one page when catching up or loading.
    #[serde(default = "consts::i64::<256>")]
    #[validate(range(min = 1))]
    pub page_size: i64,
    /// A snapshot is taken at every version that is a multiple of this.
    #[serde(default = "consts::i64::<100>")]
    #[validate(range(min = 1))]
    pub snapshot_every: i64,
    /// How many of the latest snapshots stay in the log, with the commands from the oldest of them.
    #[serde(default = "consts::i64::<3>")]
    #[validate(range(min = 1))]
    pub keep_snapshots: i64,
}

fn validate_log_config(config: &LogConfig) -> Result<(), ValidationError> {
    if config.retry_min_delay > config.retry_max_delay {
        return Err(ValidationError::new("retryDelays")
            .with_message("retryMinDelay must not be greater than retryMaxDelay".into()));
    }
    Ok(())
}

impl Default for LogConfig {
    fn default() -> Self {
        default_from_serde()
    }
}

/// Which streams of one kind may be opened. The default allows none, so a missing setting fails closed.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum StreamAccessConfig {
    /// Every stream id. For tests and local runs only.
    Any,
    /// Only the listed stream ids (without the kind, which the setting belongs to).
    AllowList { ids: Vec<String> },
}

impl Default for StreamAccessConfig {
    fn default() -> Self {
        Self::AllowList { ids: Vec::new() }
    }
}

/// Fails the config load for an id that can never be a stream id. (Written by hand: the derive of
/// `Validate` does not support enums.)
impl Validate for StreamAccessConfig {
    fn validate(&self) -> Result<(), ValidationErrors> {
        if let Self::AllowList { ids } = self {
            // The kind does not change which ids are valid.
            if let Some(id) = ids
                .iter()
                .find(|id| StreamId::new(StreamKind::Chat, id.as_str()).is_err())
            {
                let mut errors = ValidationErrors::new();
                errors.add(
                    "ids",
                    ValidationError::new("streamId").with_message(format!("invalid stream id {id:?}").into()),
                );
                return Err(errors);
            }
        }
        Ok(())
    }
}

/// The settings of the streams of one kind, grouped by the part they apply to.
#[derive(Clone, Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct StreamConfig {
    #[serde(default)]
    #[validate(nested)]
    pub inbox: InboxConfig,
    #[serde(default)]
    #[validate(nested)]
    pub log: LogConfig,

    /// Most commands waiting to be persisted before the inbox is not read any further.
    #[serde(default = "consts::usize::<512>")]
    #[validate(range(min = 1))]
    pub relay_max_pending: usize,

    /// Seconds the worker stays loaded after its last subscriber left.
    #[serde(default = "duration::seconds::<60>", with = "duration::seconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub worker_unload_grace: Duration,

    /// Which streams of the kind may be opened. Nothing is allowed unless configured.
    #[serde(default)]
    #[validate(nested)]
    pub access: StreamAccessConfig,
}

impl Default for StreamConfig {
    fn default() -> Self {
        default_from_serde()
    }
}

/// The configuration of each stream kind.
#[derive(Clone, Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct StreamsConfig {
    #[serde(default)]
    #[validate(nested)]
    pub chat: StreamConfig,
    #[serde(default)]
    #[validate(nested)]
    pub pixel_map: StreamConfig,
}

impl Default for StreamsConfig {
    fn default() -> Self {
        default_from_serde()
    }
}

#[cfg(test)]
#[path = "stream_config_test.rs"]
mod stream_config_test;
