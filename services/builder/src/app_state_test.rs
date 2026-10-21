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
