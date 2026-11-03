//! Hub behavior under Redis faults.

use crate::{
    models::messages::{DisconnectReason, HubEvent, HubMessage},
    testing::hub::{EgressState, Lifecycle, TestHub, TestHubOptions},
};
use shine_test::{
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

/// Opens idle pooled connections, so the next commands do not pay a connection handshake.

#[test(serial = "hub-service")]
async fn connect_proceeds_when_the_registry_write_landed_but_its_reply_was_lost() {
    let toxiproxy = Toxiproxy::from_env().unwrap();
    let _reset = toxiproxy.reset_guard().await;

    let mut test = TestHub::from_toxiproxy(&toxiproxy, TestHubOptions::default()).await;

    let user = Uuid::new_v4();

    let (first, mut first_rx) = test.connect_ping(user, 8);
    let _ = test.wait_all_commands().await;
    test.warmup().await;

    let proxy = toxiproxy.proxy("redis").await;
    proxy
        .add_toxic(&Toxic::limit_data("cut", Stream::Upstream, CUT_AFTER_SET))
        .await;
    let (second, mut second_rx) = test.connect_ping(user, 8);
    let first_end = EgressState::poll(&mut first_rx).await;
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
    Lifecycle::assert_by_user(
        &seen,
        user,
        &[Lifecycle::Disconnected(first), Lifecycle::Connected(second)],
    );
    EgressState::from_egress(&mut second_rx).assert_open();
    test.assert_consistent([user], &HashMap::from([(user, second)])).await;

    test.cleanup_connection(user, second).await;
}

#[test(serial = "hub-service")]
async fn connect_is_rejected_when_the_registry_write_did_not_land() {
    let toxiproxy = Toxiproxy::from_env().unwrap();
    let _reset = toxiproxy.reset_guard().await;

    let mut test = TestHub::from_toxiproxy(&toxiproxy, TestHubOptions::default()).await;
    let user = Uuid::new_v4();

    let (first, mut first_rx) = test.connect_ping(user, 8);
    test.wait_all_commands().await;
    test.warmup().await;

    let proxy = toxiproxy.proxy("redis").await;
    proxy
        .add_toxic(&Toxic::limit_data("cut", Stream::Upstream, CUT_IN_SET))
        .await;
    let (_second, mut second_rx) = test.connect_ping(user, 8);
    let second_end = EgressState::poll(&mut second_rx).await;
    proxy.remove_toxic("cut").await;

    // The new socket is released without a notice; the old connection stays intact and tracked.
    assert!(
        second_end.is_none(),
        "the rejected connection must be released: {second_end:?}"
    );
    let seen = test.wait_all_commands().await;
    Lifecycle::assert_by_user(&seen, user, &[]);
    Lifecycle::assert_drops(&seen, &[]);
    EgressState::from_egress(&mut first_rx).assert_open();
    test.assert_consistent([user], &HashMap::from([(user, first)])).await;

    test.cleanup_connection(user, first).await;
}

#[test(serial = "hub-service")]
async fn disconnect_during_an_outage_releases_the_socket_and_leaves_the_row_to_the_ttl() {
    let toxiproxy = Toxiproxy::from_env().unwrap();
    let _reset = toxiproxy.reset_guard().await;

    let mut test = TestHub::from_toxiproxy(&toxiproxy, TestHubOptions::default()).await;
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

    Lifecycle::assert_by_user(&seen, user, &[Lifecycle::Disconnected(connection)]);
    EgressState::from_egress(&mut rx)
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

// todo: #[test(serial = "hub-service")]
// async fn an_outage_keeps_live_connections_and_rejects_new_ones() {
//     let tick = Duration::from_millis(100);

//     let toxiproxy = Toxiproxy::from_env().unwrap();
//     let _reset = toxiproxy.reset_guard().await;

//     let mut test = TestHub::from_toxiproxy(
//         &toxiproxy,
//         TestHubOptions {
//             heartbeat: tick,
//             session_check: tick,
//             ..Default::default()
//         },
//     );
//     let user = Uuid::new_v4();
//     let session_key = key();
//     seed_session(&test.redis, user, session_key).await;

//     let (connection, mut rx) = connect_with_key(&test.hub, user, session_key, 8);
//     barrier(&test.hub, &mut test.events).await;

//     // Heartbeats and session checks fail for many ticks: a transient error must not disconnect anybody.
//     let proxy = toxiproxy.proxy("redis").await;
//     proxy.set_enabled(false).await;
//     let new_user = Uuid::new_v4();
//     let (_rejected, mut rejected_rx) = connect(&test.hub, new_user, 8);
//     let during = wait_event(&mut test.events, tick * 10, |_| true).await;
//     assert!(during.is_none(), "nothing may change during the outage: {during:?}");
//     assert!(
//         tokio::time::timeout(BARRIER_TIMEOUT, rejected_rx.recv())
//             .await
//             .unwrap()
//             .is_none(),
//         "a connect during the outage is rejected"
//     );
//     proxy.set_enabled(true).await;

//     // After the outage the ticks succeed again and the live connection is still there.
//     tokio::time::sleep(tick * 5).await;
//     let seen = barrier(&test.hub, &mut test.events).await;
//     assert_eq!(lifecycle_of(&seen, user), []);
//     assert_eq!(EgressSstate::from_egress(&mut rx).closed, false);
//     assert_eq!(drops(&seen), dropped(&[]));
//     assert_consistent(
//         &test.hub,
//         &registry,
//         [user, new_user],
//         &HashMap::from([(user, connection)]),
//     )
//     .await;

//     test.hub.request_disconnection(user, connection).unwrap();
//     barrier(&test.hub, &mut test.events).await;
// }
