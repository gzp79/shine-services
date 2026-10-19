use crate::{
    app_config::{CommandStreamConfig, KindRetentionConfig},
    settings::CommandStreamSettings,
};
use shine_test::test;
use std::collections::HashMap;

fn config(snapshot_every: i64, keep_snapshots: i64, kinds: &[(&str, Option<i64>, Option<i64>)]) -> CommandStreamConfig {
    CommandStreamConfig {
        snapshot_every,
        keep_snapshots,
        kinds: kinds
            .iter()
            .map(|(name, every, keep)| {
                (
                    name.to_string(),
                    KindRetentionConfig {
                        snapshot_every: *every,
                        keep_snapshots: *keep,
                    },
                )
            })
            .collect::<HashMap<_, _>>(),
        ..Default::default()
    }
}

#[test]
fn streams_are_off_by_default() {
    let config = CommandStreamConfig::default();
    assert!(!config.enabled);
    assert!(CommandStreamSettings::from_config(&config).is_ok());
}

#[test]
fn kind_overrides_win_over_the_defaults() {
    let settings = CommandStreamSettings::from_config(&config(
        100,
        3,
        &[("chat", Some(50), Some(5)), ("partial", Some(7), None)],
    ))
    .unwrap();

    let chat = settings.for_kind("chat");
    assert_eq!((chat.snapshot_every, chat.keep_snapshots), (50, 5));

    let partial = settings.for_kind("partial");
    assert_eq!(
        (partial.snapshot_every, partial.keep_snapshots),
        (7, 3),
        "unset keys keep the defaults"
    );

    let other = settings.for_kind("other");
    assert_eq!((other.snapshot_every, other.keep_snapshots), (100, 3));
}

#[test]
fn invalid_retention_fails_the_start() {
    assert!(CommandStreamSettings::from_config(&config(0, 3, &[])).is_err());
    assert!(CommandStreamSettings::from_config(&config(100, 0, &[])).is_err());
    assert!(CommandStreamSettings::from_config(&config(-1, 3, &[])).is_err());
    assert!(CommandStreamSettings::from_config(&config(100, 3, &[("chat", Some(0), None)])).is_err());
    assert!(CommandStreamSettings::from_config(&config(100, 3, &[("chat", None, Some(-2))])).is_err());
}

#[test]
fn a_snapshot_is_taken_every_snapshot_every_versions() {
    let settings = CommandStreamSettings::from_config(&config(5, 2, &[])).unwrap();
    let kind = settings.for_kind("chat");
    assert!(!kind.is_snapshot_version(0));
    assert!(!kind.is_snapshot_version(4));
    assert!(kind.is_snapshot_version(5));
    assert!(kind.is_snapshot_version(10));
}

#[test]
fn the_config_reads_camel_case_json() {
    let config: CommandStreamConfig = serde_json::from_str(
        r#"{"enabled":true,"snapshotEvery":5,"keepSnapshots":2,"syncIntervalMs":200,
            "kinds":{"chat":{"snapshotEvery":50,"keepSnapshots":5}}}"#,
    )
    .unwrap();
    assert!(config.enabled);
    let settings = CommandStreamSettings::from_config(&config).unwrap();
    assert_eq!(settings.sync_interval, std::time::Duration::from_millis(200));
    assert_eq!(settings.for_kind("chat").snapshot_every, 50);
    assert_eq!(settings.for_kind("demo").snapshot_every, 5);
}
