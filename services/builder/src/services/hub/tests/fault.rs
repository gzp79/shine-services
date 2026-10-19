//! Hub behaviour under Redis faults, injected with toxiproxy between the hub and Redis.
//!
//! Needs `SHINE_TEST_TOXIPROXY_URL` and a toxiproxy `redis` proxy in front of the test Redis
//! (`services/docker-compose.yml`); skipped otherwise. The assertions read the test Redis (which may be
//! behind the same proxy) only while no fault is active.

use crate::{
    models::messages::{DisconnectReason, HubEvent, HubMessage},
    services::hub_test_utils::{
        assert_consistent, barrier, closed_with, connect, connect_with_key, create_test_hub, delete_key,
        direct_registry, dropped, drops, egress_state, find_registry_connection, key, lifecycle_of, open, registry_key,
        seed_session, wait_event, Lifecycle, TestHub, TestHubOptions, BARRIER_TIMEOUT,
    },
};
use shine_test::{
    db::get_redis_cns,
    test,
    toxiproxy::{Stream, Toxic, Toxiproxy},
};
use std::{collections::HashMap, time::Duration};
use uuid::Uuid;

/// Upstream bytes of a registry write on a pooled connection: the checkout PING (14), the SET (127),
/// then the PUBLISH (124). Cutting in the PUBLISH applies the SET but loses the reply; cutting before
/// the SET ends applies nothing. A fresh connection adds its handshake, so the pool is warmed first.
const CUT_AFTER_SET: u64 = 200;
const CUT_IN_SET: u64 = 60;

/// A hub whose Redis goes through the proxy, with short timeouts so an outage fails fast.
async fn proxied_hub(toxiproxy: &Toxiproxy, options: TestHubOptions) -> Option<TestHub> {
    get_redis_cns()?;
    let (host, port) = (toxiproxy.host(), toxiproxy.proxy("redis").await.listen_port());
    create_test_hub(TestHubOptions {
        redis_cns: Some(format!("redis://{host}:{port}?timeout=1000&pool_timeout=500")),
        ..options
    })
    .await
}

/// Opens idle pooled connections, so the next commands do not pay a connection handshake.
async fn warm_pool(test: &TestHub) {
    let lookups: Vec<_> = (0..4)
        .map(|_| {
            let registry = test.registry.clone();
            tokio::spawn(async move { find_registry_connection(&registry, Uuid::new_v4()).await })
        })
        .collect();
    for lookup in lookups {
        lookup.await.unwrap();
    }
}

#[test(serial = "hub-service")]
async fn connect_proceeds_when_the_registry_write_landed_but_its_reply_was_lost() {
    let Some(toxiproxy) = Toxiproxy::from_env() else {
        return;
    };
    let _reset = toxiproxy.reset_guard().await;
    let Some(mut test) = proxied_hub(&toxiproxy, TestHubOptions::default()).await else {
        return;
    };
    let registry = direct_registry().await;
    let user = Uuid::new_v4();

    let (first, mut first_rx) = connect(&test.hub, user, 8);
    barrier(&test.hub, &mut test.events).await;
    warm_pool(&test).await;

    let proxy = toxiproxy.proxy("redis").await;
    proxy
        .add_toxic(&Toxic::limit_data("cut", Stream::Upstream, CUT_AFTER_SET))
        .await;
    let (second, mut second_rx) = connect(&test.hub, user, 8);
    // The replacement closes the first connection; a rejected connect would close the second one.
    let first_end = tokio::time::timeout(BARRIER_TIMEOUT, first_rx.recv()).await.unwrap();
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
    let seen = barrier(&test.hub, &mut test.events).await;
    assert_eq!(
        lifecycle_of(&seen, user),
        [Lifecycle::Disconnected(first), Lifecycle::Connected(second)]
    );
    assert_eq!(egress_state(&mut second_rx), open());
    assert_consistent(&test.hub, &registry, [user], &HashMap::from([(user, second)])).await;

    test.hub.request_disconnection(user, second).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn connect_is_rejected_when_the_registry_write_did_not_land() {
    let Some(toxiproxy) = Toxiproxy::from_env() else {
        return;
    };
    let _reset = toxiproxy.reset_guard().await;
    let Some(mut test) = proxied_hub(&toxiproxy, TestHubOptions::default()).await else {
        return;
    };
    let registry = direct_registry().await;
    let user = Uuid::new_v4();

    let (first, mut first_rx) = connect(&test.hub, user, 8);
    barrier(&test.hub, &mut test.events).await;
    warm_pool(&test).await;

    let proxy = toxiproxy.proxy("redis").await;
    proxy
        .add_toxic(&Toxic::limit_data("cut", Stream::Upstream, CUT_IN_SET))
        .await;
    let (_second, mut second_rx) = connect(&test.hub, user, 8);
    let second_end = tokio::time::timeout(BARRIER_TIMEOUT, second_rx.recv()).await.unwrap();
    proxy.remove_toxic("cut").await;

    // The new socket is released without a notice; the old connection stays intact and tracked.
    assert!(
        second_end.is_none(),
        "the rejected connection must be released: {second_end:?}"
    );
    let seen = barrier(&test.hub, &mut test.events).await;
    assert_eq!(lifecycle_of(&seen, user), []);
    assert_eq!(egress_state(&mut first_rx), open());
    assert_eq!(drops(&seen), dropped(&[]));
    assert_consistent(&test.hub, &registry, [user], &HashMap::from([(user, first)])).await;

    test.hub.request_disconnection(user, first).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn disconnect_during_an_outage_releases_the_socket_and_leaves_the_row_to_the_ttl() {
    let Some(toxiproxy) = Toxiproxy::from_env() else {
        return;
    };
    let _reset = toxiproxy.reset_guard().await;
    let Some(mut test) = proxied_hub(&toxiproxy, TestHubOptions::default()).await else {
        return;
    };
    let registry = direct_registry().await;
    let user = Uuid::new_v4();

    let (connection, mut rx) = connect(&test.hub, user, 8);
    barrier(&test.hub, &mut test.events).await;

    let proxy = toxiproxy.proxy("redis").await;
    proxy.set_enabled(false).await;
    test.hub
        .request_drop(user, connection, DisconnectReason::SessionInvalid)
        .unwrap();
    // No barrier while Redis is down: its sentinel connect would be rejected too.
    let seen = wait_event(&mut test.events, BARRIER_TIMEOUT, |_| true).await;
    proxy.set_enabled(true).await;

    assert_eq!(
        lifecycle_of(&seen.expect("the disconnect must not wedge on Redis"), user),
        [Lifecycle::Disconnected(connection)]
    );
    assert_eq!(
        egress_state(&mut rx),
        closed_with(Some(DisconnectReason::SessionInvalid))
    );
    assert_eq!(test.hub.stats().await.connections, 0);
    assert_eq!(
        find_registry_connection(&registry, user).await,
        Some(connection),
        "the registry row could not be removed; its TTL cleans it up"
    );

    delete_key(&test.redis, &registry_key(user)).await;
}

#[test(serial = "hub-service")]
async fn an_outage_keeps_live_connections_and_rejects_new_ones() {
    let Some(toxiproxy) = Toxiproxy::from_env() else {
        return;
    };
    let _reset = toxiproxy.reset_guard().await;
    let tick = Duration::from_millis(100);
    let Some(mut test) = proxied_hub(
        &toxiproxy,
        TestHubOptions {
            heartbeat: tick,
            session_check: tick,
            ..Default::default()
        },
    )
    .await
    else {
        return;
    };
    let registry = direct_registry().await;
    let user = Uuid::new_v4();
    let session_key = key();
    seed_session(&test.redis, user, session_key).await;

    let (connection, mut rx) = connect_with_key(&test.hub, user, session_key, 8);
    barrier(&test.hub, &mut test.events).await;

    // Heartbeats and session checks fail for many ticks: a transient error must not disconnect anybody.
    let proxy = toxiproxy.proxy("redis").await;
    proxy.set_enabled(false).await;
    let new_user = Uuid::new_v4();
    let (_rejected, mut rejected_rx) = connect(&test.hub, new_user, 8);
    let during = wait_event(&mut test.events, tick * 10, |_| true).await;
    assert!(during.is_none(), "nothing may change during the outage: {during:?}");
    assert!(
        tokio::time::timeout(BARRIER_TIMEOUT, rejected_rx.recv())
            .await
            .unwrap()
            .is_none(),
        "a connect during the outage is rejected"
    );
    proxy.set_enabled(true).await;

    // After the outage the ticks succeed again and the live connection is still there.
    tokio::time::sleep(tick * 5).await;
    let seen = barrier(&test.hub, &mut test.events).await;
    assert_eq!(lifecycle_of(&seen, user), []);
    assert_eq!(egress_state(&mut rx), open());
    assert_eq!(drops(&seen), dropped(&[]));
    assert_consistent(
        &test.hub,
        &registry,
        [user, new_user],
        &HashMap::from([(user, connection)]),
    )
    .await;

    test.hub.request_disconnection(user, connection).unwrap();
    barrier(&test.hub, &mut test.events).await;
}
