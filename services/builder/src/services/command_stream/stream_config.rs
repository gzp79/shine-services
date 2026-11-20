use serde::{Deserialize, Serialize};
use shine_core::{serde::default_from_serde, validation::non_zero_duration};
use shine_infra::serde::{consts, duration};
use std::time::Duration;
use validator::{Validate, ValidationError};

/// The settings of the streams of one kind, each prefixed with the part it applies to.
#[derive(Clone, Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
#[validate(schema(function = "validate_stream_config"))]
pub struct StreamConfig {
    /// Most inbox entries read in one sync.
    #[serde(default = "consts::usize::<128>")]
    #[validate(range(min = 1))]
    pub inbox_poll_limit: usize,
    /// Seconds between periodic fallback inbox checks. Notifications normally wake the worker;
    /// these occasional checks recover commands whose notifications were missed.
    #[serde(default = "duration::seconds::<10>", with = "duration::seconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub inbox_recovery_poll_interval: Duration,
    /// How many times a read of the inbox that fails because Redis is out of reach is tried again before the
    /// stream is given up; `0` gives up at once. The worker does nothing else meanwhile. With the default
    /// delays the retries last about three to five seconds, longer than a Redis restart or a failover
    /// hiccup, short enough not to keep a stream stalled on a Redis that is gone.
    #[serde(default = "consts::u32::<8>")]
    pub inbox_max_retries: u32,
    /// Milliseconds before the first retry of a failing inbox read. The waits double (and are jittered) up to
    /// `inbox_retry_max_delay`.
    #[serde(default = "duration::milliseconds::<100>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub inbox_retry_min_delay: Duration,
    /// Milliseconds the longest wait between the retries of a failing inbox read lasts.
    #[serde(default = "duration::milliseconds::<1000>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub inbox_retry_max_delay: Duration,
    /// Pushes wait once the inbox holds this many entries.
    #[serde(default = "consts::usize::<1024>")]
    pub inbox_backpressure_high: usize,
    /// Pushes wait until it is down to this many entries once the backpressure is triggered.
    #[serde(default = "consts::usize::<256>")]
    pub inbox_backpressure_low: usize,
    /// Milliseconds between the checks of the inbox length for the backpressure.
    #[serde(default = "duration::milliseconds::<250>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub inbox_backpressure_poll_period: Duration,
    /// Milliseconds a push waits for the backpressure before the connection is dropped as busy.
    #[serde(default = "duration::milliseconds::<2000>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub inbox_backpressure_timeout: Duration,

    /// How many times a call to the log that fails because Postgres is out of reach is tried again before the
    /// stream is given up; `0` gives up at once. Only the calls that are safe to repeat are retried: the reads, and
    /// the append, which the log recognises as already stored if the first try got through.
    #[serde(default = "consts::u32::<8>")]
    pub log_max_retries: u32,
    /// Milliseconds before the first retry of a failing call to the log. The waits double (and are jittered) up
    /// to `log_retry_max_delay`.
    #[serde(default = "duration::milliseconds::<100>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub log_retry_min_delay: Duration,
    /// Milliseconds the longest wait between the retries of a failing call to the log lasts.
    #[serde(default = "duration::milliseconds::<1000>", with = "duration::milliseconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub log_retry_max_delay: Duration,
    /// Most commands read from the log in one page when catching up or loading.
    #[serde(default = "consts::i64::<256>")]
    #[validate(range(min = 1))]
    pub log_page_size: i64,
    /// A snapshot is taken at every version that is a multiple of this.
    #[serde(default = "consts::i64::<100>")]
    #[validate(range(min = 1))]
    pub log_snapshot_every: i64,
    /// How many of the latest snapshots stay in the log, with the commands from the oldest of them.
    #[serde(default = "consts::i64::<3>")]
    #[validate(range(min = 1))]
    pub log_keep_snapshots: i64,

    /// Most commands waiting to be persisted before the inbox is not read any further.
    #[serde(default = "consts::usize::<512>")]
    #[validate(range(min = 1))]
    pub relay_max_pending: usize,

    /// Seconds the worker stays loaded after its last subscriber left.
    #[serde(default = "duration::seconds::<60>", with = "duration::seconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub worker_unload_grace: Duration,
}

fn validate_stream_config(config: &StreamConfig) -> Result<(), ValidationError> {
    if config.inbox_backpressure_low >= config.inbox_backpressure_high {
        return Err(ValidationError::new("watermarks")
            .with_message("inboxBackpressureLow must be less than inboxBackpressureHigh".into()));
    }
    if config.inbox_retry_min_delay > config.inbox_retry_max_delay {
        return Err(ValidationError::new("retryDelays")
            .with_message("inboxRetryMinDelay must not be greater than inboxRetryMaxDelay".into()));
    }
    if config.log_retry_min_delay > config.log_retry_max_delay {
        return Err(ValidationError::new("retryDelays")
            .with_message("logRetryMinDelay must not be greater than logRetryMaxDelay".into()));
    }
    Ok(())
}

impl Default for StreamConfig {
    fn default() -> Self {
        default_from_serde()
    }
}

impl StreamConfig {
    /// No periodic work: the recovery poll and the unload grace are an hour. The waits between the retries of
    /// a failing inbox read or log call are a few milliseconds, so a test does not wait for them.
    pub fn quiet() -> Self {
        StreamConfig {
            inbox_recovery_poll_interval: duration::seconds::<3600>(),
            inbox_retry_min_delay: duration::milliseconds::<1>(),
            inbox_retry_max_delay: duration::milliseconds::<5>(),
            log_retry_min_delay: duration::milliseconds::<1>(),
            log_retry_max_delay: duration::milliseconds::<5>(),
            worker_unload_grace: duration::seconds::<3600>(),
            ..Self::default()
        }
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

impl StreamsConfig {
    /// The same configuration for every kind.
    pub fn uniform(config: StreamConfig) -> Self {
        Self {
            chat: config.clone(),
            pixel_map: config,
        }
    }
}
