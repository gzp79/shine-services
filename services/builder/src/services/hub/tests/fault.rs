//! Hub behavior under Redis faults.

use crate::{
    models::messages::{DisconnectReason, HubEvent, HubMessage, TopicKey},
    testing::hub::{Lifecycle, TestHub, TestHubOptions, TestSession},
};
use shine_test::{
    test,
    toxiproxy::{Stream, Toxic, Toxiproxy},
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Upstream bytes of a registry write on a pooled connection: the checkout PING (14), the SET (127),
/// then the PUBLISH (124). Cutting in the PUBLISH applies the SET but loses the reply; cutting before
/// the SET ends applies nothing. A fresh connection adds its handshake, so the pool is warmed first.
const CUT_AFTER_SET: u64 = 200;
const CUT_IN_SET: u64 = 60;

/// Opens idle pooled connections, so the next commands do not pay a connection handshake.

#[test(serial = "hub-service")]
async fn connect_proceeds_when_the_registry_write_landed_but_its_reply_was_lost() {
    let toxiproxy = Toxiproxy::from_env();
    let _reset = toxiproxy.reset_guard().await;

    let mut test = TestHub::default_from_env().await;

    let user = Uuid::new_v4();

    let (first, mut first_rx) = test.connect_ping(user, 8);
    let _ = test.wait_all_commands().await;
    test.warmup().await;

    let proxy = toxiproxy.proxy("redis").await;
    proxy
        .add_toxic(&Toxic::limit_data("cut", Stream::Upstream, CUT_AFTER_SET))
        .await;
    let (second, mut second_rx) = test.connect_ping(user, 8);
    let first_end = first_rx.next().await;
    proxy.remove_toxic("cut").await;

    assert!(
        matches!(
            first_end,
            Some(HubMessage::Hub(HubEvent::UserDisconnected {
                reason: DisconnectReason::Replaced,
                ..
            }))
        ),
        "the registry already names the new connection, so it must replace the old one: {first_end:?}"
    );
    let seen = test.wait_all_commands().await;
    assert_eq!(
        seen.lifecycle_of(user),
        [Lifecycle::Disconnected(first), Lifecycle::Connected(second)]
    );
    second_rx.drain().assert_open();
    test.assert_consistent([user], &HashMap::from([(user, second)])).await;

    test.cleanup_connection(user, second).await;
}

#[test(serial = "hub-service")]
async fn connect_is_rejected_when_the_registry_write_did_not_land() {
    let toxiproxy = Toxiproxy::from_env();
    let _reset = toxiproxy.reset_guard().await;

    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();

    let (first, mut first_rx) = test.connect_ping(user, 8);
    test.wait_all_commands().await;
    test.warmup().await;

    let proxy = toxiproxy.proxy("redis").await;
    proxy
        .add_toxic(&Toxic::limit_data("cut", Stream::Upstream, CUT_IN_SET))
        .await;
    let (_second, mut second_rx) = test.connect_ping(user, 8);
    let second_end = second_rx.next().await;
    proxy.remove_toxic("cut").await;

    // The new socket is released without a notice; the old connection stays intact and tracked.
    assert!(
        second_end.is_none(),
        "the rejected connection must be released: {second_end:?}"
    );
    let seen = test.wait_all_commands().await;
    assert_eq!(seen.lifecycle_of(user), []);
    assert!(seen.drops().is_empty());
    first_rx.drain().assert_open();
    test.assert_consistent([user], &HashMap::from([(user, first)])).await;

    test.cleanup_connection(user, first).await;
}

#[test(serial = "hub-service")]
async fn disconnect_during_an_outage_releases_the_socket_and_leaves_the_row_to_the_ttl() {
    let toxiproxy = Toxiproxy::from_env();
    let _reset = toxiproxy.reset_guard().await;

    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();

    let (connection, mut rx) = test.connect_ping(user, 8);
    test.wait_all_commands().await;

    let proxy = toxiproxy.proxy("redis").await;
    proxy.set_enabled(false).await;
    test.hub
        .request_drop(user, connection, DisconnectReason::SessionInvalid)
        .unwrap();
    // No barrier while Redis is down: its sentinel connect would be rejected too.
    let seen = test
        .wait_event(None, |_| true)
        .await
        .expect("the disconnect must not wedge on Redis");
    proxy.set_enabled(true).await;

    assert_eq!(seen.lifecycle_of(user), [Lifecycle::Disconnected(connection)]);
    rx.drain()
        .assert_closed()
        .assert_disconnects(&[DisconnectReason::SessionInvalid]);

    assert_eq!(test.hub.stats().await.connections, 0);
    assert_eq!(
        test.find_connection_by_user(user).await,
        Some(connection),
        "the registry row could not be removed; its TTL cleans it up"
    );

    test.delete_connection_by_user(user).await;
}

#[test(serial = "hub-service")]
async fn an_outage_keeps_live_connections_and_rejects_new_ones() {
    let tick = Duration::from_millis(100);

    let toxiproxy = Toxiproxy::from_env();
    let _reset = toxiproxy.reset_guard().await;

    let mut test = TestHub::from_env(TestHubOptions {
        heartbeat: tick,
        session_check: tick,
        ..Default::default()
    })
    .await;
    let user = Uuid::new_v4();
    let session_key = TestSession::seed(&test.redis, user).await;

    let (connection, mut rx) = test.connect(user, 8, vec![TopicKey::Ping], Some(session_key));
    test.wait_all_commands().await;

    // Heartbeats and session checks fail for many ticks: a transient error must not disconnect anybody.
    let proxy = toxiproxy.proxy("redis").await;
    proxy.set_enabled(false).await;
    let new_user = Uuid::new_v4();
    let (_rejected, mut rejected_rx) = test.connect_ping(new_user, 8);
    let during = test.wait_event(Some(tick * 10), |_| true).await;
    assert!(during.is_none(), "nothing may change during the outage: {during:?}");
    assert!(
        rejected_rx.next().await.is_none(),
        "a connect during the outage is rejected"
    );
    proxy.set_enabled(true).await;

    // After the outage the ticks succeed again and the live connection is still there.
    tokio::time::sleep(tick * 5).await;
    let seen = test.wait_all_commands().await;
    assert_eq!(seen.lifecycle_of(user), []);
    rx.drain().assert_open();
    assert!(seen.drops().is_empty());
    test.assert_consistent([user, new_user], &HashMap::from([(user, connection)]))
        .await;

    test.cleanup_connection(user, connection).await;
}

/// With every pooled connection taken, a registry call waits for the pool (30 s by default). The loop gives up
/// after the registry timeout per call: the connect is rejected soon and the loop serves the next user.
#[test(serial = "hub-service")]
async fn an_exhausted_pool_holds_the_loop_for_the_registry_timeout_only() {
    let mut test = TestHub::from_env(TestHubOptions {
        registry_timeout: Duration::from_millis(200),
        ..Default::default()
    })
    .await;

    let pool = test.redis.pool.clone();
    let mut held = Vec::new();
    while let Ok(Ok(connection)) = tokio::time::timeout(Duration::from_millis(300), pool.get()).await {
        held.push(connection);
    }
    let started = Instant::now();
    let user = Uuid::new_v4();
    let (_rejected, mut rejected_rx) = test.connect_ping(user, 8);
    let rejected = rejected_rx.next().await;
    let took = started.elapsed();
    drop(held);

    assert!(rejected.is_none(), "the connect is rejected: {rejected:?}");
    assert!(took < Duration::from_secs(3), "the loop waited {took:?} for the pool");

    let other = Uuid::new_v4();
    let (connection, mut other_rx) = test.connect_ping(other, 8);
    let seen = test.wait_all_commands().await;
    assert_eq!(seen.lifecycle_of(other), [Lifecycle::Connected(connection)]);
    other_rx.drain().assert_open();
    test.assert_consistent([user, other], &HashMap::from([(other, connection)]))
        .await;

    test.cleanup_connection(other, connection).await;
}

/// A slow registry must not make a shutdown take as long as all the connections added up: the sockets are
/// told at once and the rows are removed side by side.
#[test(serial = "hub-service")]
async fn shutdown_with_a_slow_registry_takes_no_longer_than_one_round_of_removals() {
    const USERS: usize = 30;
    const LATENCY_MS: u64 = 300;

    let toxiproxy = Toxiproxy::from_env();
    let _reset = toxiproxy.reset_guard().await;

    let mut test = TestHub::default_from_env().await;
    let users: Vec<Uuid> = (0..USERS).map(|_| Uuid::new_v4()).collect();
    let mut receivers: Vec<_> = users.iter().map(|user| test.connect_ping(*user, 8).1).collect();
    test.wait_all_commands().await;

    let proxy = toxiproxy.proxy("redis").await;
    proxy
        .add_toxic(&Toxic::latency("slow", Stream::Downstream, LATENCY_MS))
        .await;

    let started = std::time::Instant::now();
    test.hub.request_shutdown().unwrap();
    test.wait_for_shutdown_events().await;
    test.wait_shutdown_complete().await;
    let took = started.elapsed();
    proxy.remove_toxic("slow").await;

    assert!(
        took < Duration::from_secs(5),
        "the shutdown took {took:?}, the removals ran one after the other ({USERS} x {LATENCY_MS} ms at least)"
    );
    for rx in &mut receivers {
        rx.drain()
            .assert_closed()
            .assert_disconnects(&[DisconnectReason::Shutdown]);
    }
    for user in users {
        assert_eq!(test.find_connection_by_user(user).await, None, "a registry row is left");
    }
}
