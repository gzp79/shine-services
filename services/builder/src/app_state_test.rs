//! How the configuration becomes the hub's cadences: the mapping the hub tests bypass by building the hub directly.

use super::HubTiming;
use crate::{
    app_config::{AppConfig, CommandStreamConfig, WsConfig},
    repositories::DBConfig,
    services::HubIntervals,
};
use shine_test::test;
use std::time::Duration;

fn config(hub_heartbeat_seconds: u64, auth_check_interval: u64) -> AppConfig {
    AppConfig {
        db: DBConfig {
            redis_cns: String::new(),
            sql_cns: None,
        },
        ws: WsConfig { allowed_hosts: vec![] },
        command_stream: CommandStreamConfig::default(),
        hub_heartbeat_seconds,
        auth_check_interval,
    }
}

#[test]
fn the_intervals_and_the_registry_ttl_follow_the_configuration() {
    assert_eq!(
        HubTiming::from_config(&config(300, 60)),
        HubTiming {
            intervals: HubIntervals {
                heartbeat: Duration::from_secs(300),
                session_check: Duration::from_secs(60),
            },
            // A missed refresh must not expire a live row: three heartbeats.
            connection_ttl_seconds: 900,
        }
    );
}

#[test]
fn zero_intervals_are_raised_to_one_second() {
    let timing = HubTiming::from_config(&config(0, 0));
    assert_eq!(timing.intervals.heartbeat, Duration::from_secs(1));
    assert_eq!(timing.intervals.session_check, Duration::from_secs(1));
    assert_eq!(timing.connection_ttl_seconds, 3);
}

#[test]
fn a_huge_heartbeat_does_not_overflow_the_ttl() {
    assert_eq!(
        HubTiming::from_config(&config(u64::MAX, 1)).connection_ttl_seconds,
        u64::MAX
    );
}

#[test]
fn the_shipped_configuration_keeps_a_live_row_alive() {
    // Whatever the intervals, the row outlives a heartbeat by a margin.
    for (heartbeat, ttl) in [(300, 900), (1, 3)] {
        let timing = HubTiming::from_config(&config(heartbeat, 2));
        assert!(timing.connection_ttl_seconds > timing.intervals.heartbeat.as_secs());
        assert_eq!(timing.connection_ttl_seconds, ttl);
    }
}
