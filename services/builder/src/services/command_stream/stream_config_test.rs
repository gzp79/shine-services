use crate::services::command_stream::{InboxConfig, LogConfig, StreamAccessConfig, StreamConfig, StreamsConfig};
use shine_infra::serde::duration;
use shine_test::test;
use validator::Validate;

impl StreamConfig {
    /// No periodic work: the recovery poll and the unload grace are an hour. The waits between the retries of
    /// a failing inbox read or log call are a few milliseconds, so a test does not wait for them. Every
    /// stream may be opened.
    pub fn test_default() -> Self {
        StreamConfig {
            access: StreamAccessConfig::Any,
            inbox: InboxConfig {
                recovery_poll_interval: duration::seconds::<3600>(),
                retry_min_delay: duration::milliseconds::<1>(),
                retry_max_delay: duration::milliseconds::<5>(),
                ..InboxConfig::default()
            },
            log: LogConfig {
                retry_min_delay: duration::milliseconds::<1>(),
                retry_max_delay: duration::milliseconds::<5>(),
                ..LogConfig::default()
            },
            worker_unload_grace: duration::seconds::<3600>(),
            ..Self::default()
        }
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

fn allow(ids: &[&str]) -> StreamAccessConfig {
    StreamAccessConfig::AllowList {
        ids: ids.iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn a_missing_access_setting_allows_nothing() {
    let streams: StreamsConfig = serde_json::from_str(r#"{"chat":{"access":{"mode":"any"}}}"#).unwrap();

    assert_eq!(streams.chat.access, StreamAccessConfig::Any);
    assert_eq!(streams.pixel_map.access, allow(&[]));
    assert_eq!(StreamConfig::default().access, allow(&[]));
}

#[test]
fn the_settings_of_the_inbox_and_the_log_are_read_by_their_short_names() {
    let streams: StreamsConfig = serde_json::from_str(
        r#"{"chat":{"inbox":{"recoveryPollInterval":1,"pollLimit":7},"log":{"snapshotEvery":5}}}"#,
    )
    .unwrap();

    assert_eq!(streams.chat.inbox.recovery_poll_interval.as_secs(), 1);
    assert_eq!(streams.chat.inbox.poll_limit, 7);
    assert_eq!(streams.chat.log.snapshot_every, 5);
    assert_eq!(streams.chat.log.page_size, StreamConfig::default().log.page_size);
}

#[test]
fn an_invalid_id_fails_the_config_validation() {
    assert!(allow(&["global"]).validate().is_ok());
    assert!(StreamAccessConfig::Any.validate().is_ok());

    let config = StreamConfig {
        access: allow(&["global", "bad id"]),
        ..StreamConfig::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn inverted_retry_delays_or_watermarks_fail_the_config_validation() {
    let mut config = StreamConfig::default();
    assert!(config.validate().is_ok());

    config.inbox.backpressure_low = config.inbox.backpressure_high;
    assert!(config.validate().is_err());

    let mut config = StreamConfig::default();
    config.log.retry_min_delay = config.log.retry_max_delay * 2;
    assert!(config.validate().is_err());
}
