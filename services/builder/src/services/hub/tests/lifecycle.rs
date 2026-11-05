//! Deterministic edge cases of the connection lifecycle on one hub.

use crate::{
    models::messages::{DisconnectReason, HubEvent, HubMessage, TopicKey},
    repositories::hub_registry::{HubConnection, HubConnectionDb, HubRegistry},
    testing::{
        hub::{Lifecycle, TestHub, TestHubOptions, TestSession},
        TestMessage,
    },
};
use ring::rand::SystemRandom;
use shine_infra::session::SessionKey;
use shine_test::test;
use std::collections::BTreeMap;
use std::{collections::HashMap, time::Duration};
use uuid::Uuid;

#[test(serial = "hub-service")]
async fn replaced_connection_gets_close_notice_and_one_drop() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();
    let (first, mut first_rx) = test.connect_ping(user, 8);
    let (second, mut second_rx) = test.connect_ping(user, 8);
    let seen = test.wait_all_commands().await;

    assert_eq!(
        seen.lifecycle_of(user),
        [
            Lifecycle::Connected(first),
            Lifecycle::Disconnected(first),
            Lifecycle::Connected(second),
        ]
    );

    first_rx
        .drain()
        .assert_closed()
        .assert_disconnects(&[DisconnectReason::Replaced]);
    second_rx.drain().assert_open();
    assert_eq!(seen.drops(), BTreeMap::from([(DisconnectReason::Replaced, 1)]));
    test.assert_consistent([user], &HashMap::from([(user, second)])).await;

    test.cleanup_connection(user, second).await;
    test.assert_consistent([user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn stale_unknown_and_repeated_disconnects_are_noops() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();

    // Unknown user and unknown connection.
    test.hub.request_disconnection(Uuid::new_v4(), Uuid::new_v4()).unwrap();
    test.hub
        .request_drop(user, Uuid::new_v4(), DisconnectReason::SessionInvalid)
        .unwrap();
    let (first, _first_rx) = test.connect_ping(user, 8);
    let (second, mut second_rx) = test.connect_ping(user, 8);
    // Stale ids of the replaced connection must not touch the active one.
    test.hub.request_disconnection(user, first).unwrap();
    test.hub
        .request_drop(user, first, DisconnectReason::SessionInvalid)
        .unwrap();
    test.hub.request_disconnection(user, second).unwrap();
    // Repeats after the removal are no-ops, so a late forced drop is not counted.
    test.hub.request_disconnection(user, second).unwrap();
    test.hub
        .request_drop(user, second, DisconnectReason::HeartbeatInactive)
        .unwrap();
    let seen = test.wait_all_commands().await;

    assert_eq!(
        seen.lifecycles(),
        HashMap::from([(
            user,
            vec![
                Lifecycle::Connected(first),
                Lifecycle::Disconnected(first),
                Lifecycle::Connected(second),
                Lifecycle::Disconnected(second)
            ]
        )])
    );
    second_rx.drain().assert_closed();
    assert_eq!(seen.drops(), BTreeMap::from([(DisconnectReason::Replaced, 1)]));
    test.assert_consistent([user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn forced_drop_tells_the_socket_why() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();

    let (connection, mut rx) = test.connect_ping(user, 8);
    test.hub
        .request_drop(user, connection, DisconnectReason::SessionInvalid)
        .unwrap();
    let seen = test.wait_all_commands().await;

    assert_eq!(
        seen.lifecycle_of(user),
        [Lifecycle::Connected(connection), Lifecycle::Disconnected(connection)]
    );

    rx.drain()
        .assert_closed()
        .assert_disconnects(&[DisconnectReason::SessionInvalid]);
    assert_eq!(seen.drops(), BTreeMap::from([(DisconnectReason::SessionInvalid, 1)]));
    test.assert_consistent([user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn client_close_while_connect_is_queued_is_not_a_drop() {
    let mut test = TestHub::default_from_env().await;

    // The socket closes before the hub has even processed its connect, as a ws client that goes away
    // right after the upgrade. Off the Hub topic, nothing is sent to it in between.
    let user = Uuid::new_v4();
    let (connection, rx) = test.connect_ping(user, 8);
    drop(rx);
    test.hub.request_disconnection(user, connection).unwrap();
    let seen = test.wait_all_commands().await;

    assert_eq!(
        seen.lifecycle_of(user),
        [Lifecycle::Connected(connection), Lifecycle::Disconnected(connection)]
    );
    assert!(seen.drops().is_empty());

    // On the Hub topic its own UserConnected broadcast finds the closed channel: the same client
    // close is counted as a dead channel. A ws socket avoids this by keeping its egress open until the hub
    // releases it.
    let hub_user = Uuid::new_v4();
    let (hub_connection, hub_rx) = test.connect(hub_user, 8, vec![TopicKey::Hub], None);
    drop(hub_rx);
    test.hub.request_disconnection(hub_user, hub_connection).unwrap();
    let seen = test.wait_all_commands().await;

    assert_eq!(
        seen.lifecycle_of(hub_user),
        [
            Lifecycle::Connected(hub_connection),
            Lifecycle::Disconnected(hub_connection),
        ]
    );
    assert_eq!(seen.drops(), BTreeMap::from([(DisconnectReason::DeadChannel, 1)]));
    test.assert_consistent([user, hub_user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn dead_channel_is_pruned_once() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();

    let (connection, rx) = test.connect_ping(user, 8);
    test.wait_all_commands().await;
    drop(rx);

    // Every send finds the channel closed and queues a prune; only the first one removes it.
    for idx in 0..3 {
        assert!(!test
            .send_to_connection(connection, TestMessage::new_ping(format!("d-{idx}")))
            .await
            .unwrap());
        test.broadcast(TestMessage::new_ping(format!("b-{idx}"))).await.unwrap();
    }
    let seen = test.wait_all_commands().await;

    assert_eq!(seen.lifecycle_of(user), [Lifecycle::Disconnected(connection)]);
    assert_eq!(seen.drops(), BTreeMap::from([(DisconnectReason::DeadChannel, 1)]));
    test.assert_consistent([user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn slow_consumer_is_dropped_and_its_channel_released() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();

    let (connection, mut rx) = test.connect_ping(user, 2);
    test.wait_all_commands().await;

    // The first two fill the buffer, the rest find it full. Broadcasts are sent off the loop, so
    // more of them may still find the connection before the prune is processed.
    for idx in 0..5 {
        test.broadcast(TestMessage::new_ping(format!("b-{idx}"))).await.unwrap();
    }
    let seen = test.wait_all_commands().await;

    assert_eq!(seen.lifecycle_of(user), [Lifecycle::Disconnected(connection)]);
    rx.drain()
        .assert_closed()
        .assert_pings(&["b-0", "b-1"])
        .assert_disconnects(&[]);
    assert_eq!(seen.drops(), BTreeMap::from([(DisconnectReason::SlowConsumer, 1)]));
    test.assert_consistent([user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn send_to_user_reaches_only_the_active_connection() {
    let mut test = TestHub::default_from_env().await;
    let (user, other) = (Uuid::new_v4(), Uuid::new_v4());

    // Unknown user: nothing to send to, not an error.
    test.send_to_user(user, TestMessage::new_ping("to-nobody"))
        .await
        .unwrap();

    let (_first, mut first_rx) = test.connect_ping(user, 8);
    test.wait_all_commands().await;
    test.send_to_user(user, TestMessage::new_ping("to-first"))
        .await
        .unwrap();

    // After a replacement the user's messages follow the new connection only.
    let (second, mut second_rx) = test.connect_ping(user, 8);
    let (_other, mut other_rx) = test.connect_ping(other, 8);
    test.wait_all_commands().await;
    test.send_to_user(user, TestMessage::new_ping("to-second"))
        .await
        .unwrap();

    // Once disconnected, the user is unknown again.
    test.hub.request_disconnection(user, second).unwrap();
    test.wait_all_commands().await;
    test.send_to_user(user, TestMessage::new_ping("too-late"))
        .await
        .unwrap();
    test.wait_all_commands().await;

    first_rx
        .drain()
        .assert_pings(&["to-first"])
        .assert_disconnects(&[DisconnectReason::Replaced])
        .assert_closed();
    second_rx
        .drain()
        .assert_pings(&["to-second"])
        .assert_disconnects(&[])
        .assert_closed();
    other_rx.drain().assert_open();
}

#[test(serial = "hub-service")]
async fn send_to_user_prunes_a_dead_or_slow_connection_once() {
    let mut test = TestHub::default_from_env().await;
    let (gone_user, slow_user) = (Uuid::new_v4(), Uuid::new_v4());

    let (gone, gone_rx) = test.connect_ping(gone_user, 8);
    let (slow, mut slow_rx) = test.connect_ping(slow_user, 1);
    test.wait_all_commands().await;
    drop(gone_rx);

    // Every send finds the problem and queues a prune; only the first one removes the connection.
    for idx in 0..3 {
        test.send_to_user(gone_user, TestMessage::new_ping(format!("g-{idx}")))
            .await
            .unwrap();
        test.send_to_user(slow_user, TestMessage::new_ping(format!("s-{idx}")))
            .await
            .unwrap();
    }
    let seen = test.wait_all_commands().await;

    assert_eq!(seen.lifecycle_of(gone_user), [Lifecycle::Disconnected(gone)]);
    assert_eq!(seen.lifecycle_of(slow_user), [Lifecycle::Disconnected(slow)]);
    assert_eq!(
        seen.drops(),
        BTreeMap::from([(DisconnectReason::DeadChannel, 1), (DisconnectReason::SlowConsumer, 1)])
    );
    slow_rx
        .drain()
        .assert_pings(&["s-0"])
        .assert_disconnects(&[])
        .assert_closed();
    test.assert_consistent([gone_user, slow_user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn directed_and_broadcast_sends_reach_only_their_targets() {
    let mut test = TestHub::default_from_env().await;
    let (user, other, hub_only) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());

    let (first, mut first_rx) = test.connect_ping(user, 8);
    let (second, mut second_rx) = test.connect_ping(user, 8);
    let (_other, mut other_rx) = test.connect_ping(other, 8);
    let (_hub_only, mut hub_only_rx) = test.connect(hub_only, 8, vec![TopicKey::Hub], None);
    test.wait_all_commands().await;

    assert!(!test
        .send_to_connection(first, TestMessage::new_ping("to-first"))
        .await
        .ok()
        .unwrap());
    assert!(test
        .send_to_connection(second, TestMessage::new_ping("to-second"))
        .await
        .ok()
        .unwrap());
    test.broadcast(TestMessage::new_ping("to-all")).await.unwrap();
    test.wait_all_commands().await;

    first_rx
        .drain()
        .assert_disconnects(&[DisconnectReason::Replaced])
        .assert_closed();
    second_rx.drain().assert_pings(&["to-second", "to-all"]).assert_open();
    other_rx.drain().assert_pings(&["to-all"]).assert_open();
    hub_only_rx.drain().assert_pings(&[]).assert_open();
}

#[test(serial = "hub-service")]
async fn a_late_subscriber_sees_only_later_events() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();

    let (first, _first_rx) = test.connect_ping(user, 8);
    test.wait_all_commands().await;
    let mut late = test.hub.subscribe(vec![TopicKey::Hub]).await;
    let (second, _second_rx) = test.connect_ping(user, 8);
    test.wait_all_commands().await;

    let late_seen = test.wait_all_commands_on(&mut late).await;
    assert_eq!(
        late_seen.lifecycle_of(user),
        [Lifecycle::Disconnected(first), Lifecycle::Connected(second)]
    );
}

#[test(serial = "hub-service")]
async fn shutdown_drops_every_connection_and_publishes_nothing_after() {
    let mut test = TestHub::default_from_env().await;
    let users: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

    let mut receivers: Vec<_> = users.iter().map(|user| test.connect_ping(*user, 8).1).collect();
    test.wait_all_commands().await;

    test.hub.request_shutdown().unwrap();
    // Commands queued behind the Shutdown are never processed.
    let late_user = Uuid::new_v4();
    let _ = test.hub.request_connection(
        late_user,
        SessionKey::new_random(&SystemRandom::new()).unwrap(),
        tokio::sync::mpsc::channel(1).0,
        vec![],
    );

    // The subscribers are released after the Shutdown event, so the stream ends right after it.
    let after = test.wait_for_shutdown_events().await;
    assert!(
        matches!(after.as_slice(), [HubMessage::Hub(HubEvent::Shutdown)]),
        "only the Shutdown event is published: {after:?}"
    );
    test.wait_shutdown_complete().await;

    for rx in &mut receivers {
        rx.drain()
            .assert_closed()
            .assert_disconnects(&[DisconnectReason::Shutdown]);
    }
    assert_eq!(test.hub.stats().await.subscribers, 0);
    test.assert_consistent(users.into_iter().chain([late_user]), &HashMap::new())
        .await;
}

#[test(serial = "hub-service")]
async fn a_stopped_hub_refuses_every_request() {
    let mut test = TestHub::default_from_env().await;
    let (user, connection) = (Uuid::new_v4(), Uuid::new_v4());
    test.hub.request_shutdown().unwrap();
    test.wait_for_shutdown_events().await;
    test.wait_shutdown_complete().await;

    // A socket that connects while the hub stops must be able to tell, to close itself.
    let session = SessionKey::new_random(&SystemRandom::new()).unwrap();
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    assert!(test.hub.request_connection(user, session, tx, vec![]).is_err());
    assert!(test.hub.request_disconnection(user, connection).is_err());
    assert!(test.hub.request_shutdown().is_err());
    // Waiting for a stop that already happened returns at once.
    test.wait_shutdown_complete().await;
}

#[test(serial = "hub-service")]
async fn registry_change_drops_only_a_connection_the_registry_no_longer_holds() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();
    let (connection, mut rx) = test.connect_ping(user, 8);
    test.wait_all_commands().await;

    // A change notice while the registry still holds this connection is a no-op.
    test.hub.notify_registry_changed(user).unwrap();
    test.hub.notify_registry_changed(Uuid::new_v4()).unwrap();
    let seen = test.wait_all_commands().await;
    assert_eq!(seen.lifecycle_of(user), []);
    rx.drain().assert_open();

    // Another instance took the user's slot: the local connection is dropped, the winner kept.
    let winner = Uuid::new_v4();
    {
        let mut context = test.direct_registry.create_context().await.unwrap();
        context.create_connection(user, winner).await.unwrap();
    }
    test.hub.notify_registry_changed(user).unwrap();
    let seen = test.wait_all_commands().await;

    assert_eq!(seen.lifecycle_of(user), [Lifecycle::Disconnected(connection)]);
    rx.drain()
        .assert_closed()
        .assert_disconnects(&[DisconnectReason::RegistryInactive]);
    assert_eq!(seen.drops(), BTreeMap::from([(DisconnectReason::RegistryInactive, 1)]));
    assert_eq!(test.find_connection_by_user(user).await, Some(winner));
    test.delete_connection_by_user(user).await;
}

#[test(serial = "hub-service")]
async fn heartbeat_refreshes_live_rows_and_drops_expired_ones() {
    let tick = Duration::from_millis(100);
    let mut test = TestHub::from_env(TestHubOptions {
        heartbeat: tick,
        ..Default::default()
    })
    .await;
    let (live_user, expired_user) = (Uuid::new_v4(), Uuid::new_v4());
    let (live, mut live_rx) = test.connect_ping(live_user, 8);
    let (expired, mut expired_rx) = test.connect_ping(expired_user, 8);
    test.wait_all_commands().await;

    // One row about to expire, the other already gone, as after a TTL expiry.
    test.set_connection_ttl(live_user, 5).await;
    test.delete_connection_by_user(expired_user).await;

    let seen = test
        .wait_event(
            None,
            |event| matches!(event, HubEvent::UserDisconnected { connection_id, .. } if *connection_id == expired),
        )
        .await
        .expect("the heartbeat must drop the connection whose row expired");
    assert_eq!(seen.lifecycle_of(live_user), []);
    tokio::time::sleep(tick * 3).await;
    test.wait_all_commands().await;

    expired_rx
        .drain()
        .assert_closed()
        .assert_disconnects(&[DisconnectReason::HeartbeatInactive]);
    live_rx.drain().assert_open();
    let ttl = test.get_connection_ttl(live_user).await;
    assert!(ttl > 5, "the heartbeat must refresh the TTL of a live row, got {ttl}");
    test.assert_consistent([live_user, expired_user], &HashMap::from([(live_user, live)]))
        .await;

    test.hub.request_disconnection(live_user, live).unwrap();
    test.wait_all_commands().await;
}

#[test(serial = "hub-service")]
async fn session_check_drops_only_connections_with_an_invalid_session() {
    let tick = Duration::from_millis(100);
    let mut test = TestHub::from_env(TestHubOptions {
        session_check: tick,
        ..Default::default()
    })
    .await;
    let (valid_user, invalid_user) = (Uuid::new_v4(), Uuid::new_v4());

    let valid_key = TestSession::seed(&test.redis, valid_user).await;
    let (valid, mut valid_rx) = test.connect(valid_user, 8, vec![TopicKey::Ping], Some(valid_key));

    let (invalid, mut invalid_rx) = test.connect_ping(invalid_user, 8);

    let seen = test
        .wait_event(
            None,
            |event| matches!(event, HubEvent::UserDisconnected { connection_id, .. } if *connection_id == invalid),
        )
        .await
        .expect("the session check must drop the connection without a session");
    tokio::time::sleep(tick * 3).await;
    let mut seen = seen;
    seen.extend(test.wait_all_commands().await);

    assert_eq!(seen.lifecycle_of(valid_user), [Lifecycle::Connected(valid)]);
    invalid_rx
        .drain()
        .assert_closed()
        .assert_disconnects(&[DisconnectReason::SessionInvalid]);
    valid_rx.drain().assert_open();
    assert_eq!(seen.drops(), BTreeMap::from([(DisconnectReason::SessionInvalid, 1)]));

    test.hub.request_disconnection(valid_user, valid).unwrap();
    test.wait_all_commands().await;
}

#[test(serial = "hub-service")]
async fn batched_heartbeat_refreshes_active_and_reports_stale() {
    let mut test = TestHub::default_from_env().await;
    let (active_a, active_b, never_registered) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let (conn_a, _rx_a) = test.connect_ping(active_a, 8);
    let (conn_b, _rx_b) = test.connect_ping(active_b, 8);
    test.wait_all_commands().await;

    // The active id, a stale id of an active user, and a user without a row.
    let requested = [
        HubConnection {
            user_id: active_a,
            connection_id: conn_a,
        },
        HubConnection {
            user_id: active_b,
            connection_id: Uuid::new_v4(),
        },
        HubConnection {
            user_id: never_registered,
            connection_id: Uuid::new_v4(),
        },
    ];
    let mut stale = test.hub.heartbeat_registry_connections(&requested).await.unwrap();
    stale.sort_by_key(|c| c.user_id);
    let mut expected = vec![requested[1], requested[2]];
    expected.sort_by_key(|c| c.user_id);
    assert_eq!(
        stale, expected,
        "only the mismatching / missing entries are reported stale"
    );

    assert!(test.hub.heartbeat_registry_connection(active_a, conn_a).await.unwrap());
    assert!(!test
        .hub
        .heartbeat_registry_connection(active_a, Uuid::new_v4())
        .await
        .unwrap());

    test.hub.request_disconnection(active_a, conn_a).unwrap();
    test.hub.request_disconnection(active_b, conn_b).unwrap();
    test.wait_all_commands().await;
}
