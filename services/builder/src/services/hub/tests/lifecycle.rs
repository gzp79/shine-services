//! Deterministic edge cases of the connection lifecycle on one hub: every scenario ends in a [`barrier`],
//! then checks the exact events, egress channels, drop totals and registry.

use crate::{
    models::messages::{DisconnectReason, HubEvent, HubMessage, TopicKey},
    repositories::hub_registry::{HubConnection, HubConnectionDb, HubRegistry},
    services::hub_test_utils::{
        assert_consistent, barrier, closed_with, connect, connect_on, connect_with_key, create_test_hub, delete_key,
        dropped, drops, egress_state, find_registry_connection, key, lifecycle_by_user, lifecycle_of, open, ping,
        registry_key, seed_session, wait_event, wait_stopped, EgressState, Lifecycle, TestHub, TestHubOptions,
        BARRIER_TIMEOUT,
    },
};
use shine_test::test;
use std::{collections::HashMap, time::Duration};
use uuid::Uuid;

async fn hub() -> Option<TestHub> {
    create_test_hub(TestHubOptions::default()).await
}

#[test(serial = "hub-service")]
async fn replaced_connection_gets_close_notice_and_one_drop() {
    let Some(mut test) = hub().await else {
        return;
    };
    let user = Uuid::new_v4();

    let (first, mut first_rx) = connect(&test.hub, user, 8);
    let (second, mut second_rx) = connect(&test.hub, user, 8);
    let seen = barrier(&test.hub, &mut test.events).await;

    assert_eq!(
        lifecycle_of(&seen, user),
        [
            Lifecycle::Connected(first),
            Lifecycle::Disconnected(first),
            Lifecycle::Connected(second)
        ]
    );
    assert_eq!(
        egress_state(&mut first_rx),
        closed_with(Some(DisconnectReason::Replaced))
    );
    assert_eq!(egress_state(&mut second_rx), open());
    assert_eq!(drops(&seen), dropped(&[(DisconnectReason::Replaced, 1)]));
    assert_consistent(&test.hub, &test.registry, [user], &HashMap::from([(user, second)])).await;

    test.hub.request_disconnection(user, second).unwrap();
    barrier(&test.hub, &mut test.events).await;
    assert_consistent(&test.hub, &test.registry, [user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn stale_unknown_and_repeated_disconnects_are_noops() {
    let Some(mut test) = hub().await else {
        return;
    };
    let user = Uuid::new_v4();

    // Unknown user and unknown connection.
    test.hub.request_disconnection(Uuid::new_v4(), Uuid::new_v4()).unwrap();
    test.hub
        .request_drop(user, Uuid::new_v4(), DisconnectReason::SessionInvalid)
        .unwrap();
    let (first, _first_rx) = connect(&test.hub, user, 8);
    let (second, mut second_rx) = connect(&test.hub, user, 8);
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
    let seen = barrier(&test.hub, &mut test.events).await;

    assert_eq!(
        lifecycle_by_user(&seen),
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
    assert_eq!(egress_state(&mut second_rx), closed_with(None));
    assert_eq!(drops(&seen), dropped(&[(DisconnectReason::Replaced, 1)]));
    assert_consistent(&test.hub, &test.registry, [user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn forced_drop_tells_the_socket_why() {
    let Some(mut test) = hub().await else {
        return;
    };
    let user = Uuid::new_v4();

    let (connection, mut rx) = connect(&test.hub, user, 8);
    test.hub
        .request_drop(user, connection, DisconnectReason::SessionInvalid)
        .unwrap();
    let seen = barrier(&test.hub, &mut test.events).await;

    assert_eq!(
        lifecycle_of(&seen, user),
        [Lifecycle::Connected(connection), Lifecycle::Disconnected(connection)]
    );
    assert_eq!(
        egress_state(&mut rx),
        closed_with(Some(DisconnectReason::SessionInvalid))
    );
    assert_eq!(drops(&seen), dropped(&[(DisconnectReason::SessionInvalid, 1)]));
    assert_consistent(&test.hub, &test.registry, [user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn client_close_while_connect_is_queued_is_not_a_drop() {
    let Some(mut test) = hub().await else {
        return;
    };

    // The socket closes before the hub has even processed its connect, as a ws client that goes away
    // right after the upgrade. Off the Hub topic, nothing is sent to it in between.
    let user = Uuid::new_v4();
    let (connection, rx) = connect(&test.hub, user, 8);
    drop(rx);
    test.hub.request_disconnection(user, connection).unwrap();
    let seen = barrier(&test.hub, &mut test.events).await;

    assert_eq!(
        lifecycle_of(&seen, user),
        [Lifecycle::Connected(connection), Lifecycle::Disconnected(connection)]
    );
    assert_eq!(drops(&seen), dropped(&[]));

    // On the Hub topic its own UserConnected broadcast finds the closed channel: the same client
    // close is counted as a dead channel. A ws socket avoids this by keeping its egress open until the hub
    // releases it.
    let hub_user = Uuid::new_v4();
    let (hub_connection, hub_rx) = connect_on(&test.hub, hub_user, 8, vec![TopicKey::Hub]);
    drop(hub_rx);
    test.hub.request_disconnection(hub_user, hub_connection).unwrap();
    let seen = barrier(&test.hub, &mut test.events).await;

    assert_eq!(
        lifecycle_of(&seen, hub_user),
        [
            Lifecycle::Connected(hub_connection),
            Lifecycle::Disconnected(hub_connection)
        ]
    );
    assert_eq!(drops(&seen), dropped(&[(DisconnectReason::DeadChannel, 1)]));
    assert_consistent(&test.hub, &test.registry, [user, hub_user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn dead_channel_is_pruned_once() {
    let Some(mut test) = hub().await else {
        return;
    };
    let user = Uuid::new_v4();

    let (connection, rx) = connect(&test.hub, user, 8);
    barrier(&test.hub, &mut test.events).await;
    drop(rx);

    // Every send finds the channel closed and queues a prune; only the first one removes it.
    for idx in 0..3 {
        assert_eq!(
            test.hub
                .send_to_connection(connection, ping(format!("d-{idx}")))
                .await
                .ok(),
            Some(false)
        );
        test.hub.broadcast(ping(format!("b-{idx}"))).await.unwrap();
    }
    let seen = barrier(&test.hub, &mut test.events).await;

    assert_eq!(lifecycle_of(&seen, user), [Lifecycle::Disconnected(connection)]);
    assert_eq!(drops(&seen), dropped(&[(DisconnectReason::DeadChannel, 1)]));
    assert_consistent(&test.hub, &test.registry, [user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn slow_consumer_is_dropped_and_its_channel_released() {
    let Some(mut test) = hub().await else {
        return;
    };
    let user = Uuid::new_v4();

    let (connection, mut rx) = connect(&test.hub, user, 2);
    barrier(&test.hub, &mut test.events).await;

    // The first two fill the buffer, the rest find it full. Broadcasts are sent off the loop, so
    // more of them may still find the connection before the prune is processed.
    for idx in 0..5 {
        test.hub.broadcast(ping(format!("b-{idx}"))).await.unwrap();
    }
    let seen = barrier(&test.hub, &mut test.events).await;

    assert_eq!(lifecycle_of(&seen, user), [Lifecycle::Disconnected(connection)]);
    // The buffered messages are still delivered; the close notice did not fit, the channel closing
    // ends the socket.
    assert_eq!(
        egress_state(&mut rx),
        EgressState {
            pings: vec!["b-0".into(), "b-1".into()],
            notices: vec![],
            closed: true,
        }
    );
    assert_eq!(drops(&seen), dropped(&[(DisconnectReason::SlowConsumer, 1)]));
    assert_consistent(&test.hub, &test.registry, [user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn send_to_user_reaches_only_the_active_connection() {
    let Some(mut test) = hub().await else {
        return;
    };
    let (user, other) = (Uuid::new_v4(), Uuid::new_v4());

    // Unknown user: nothing to send to, not an error.
    test.hub.send_to_user(user, ping("to-nobody")).await.unwrap();

    let (_first, mut first_rx) = connect(&test.hub, user, 8);
    barrier(&test.hub, &mut test.events).await;
    test.hub.send_to_user(user, ping("to-first")).await.unwrap();

    // After a replacement the user's messages follow the new connection only.
    let (second, mut second_rx) = connect(&test.hub, user, 8);
    let (_other, mut other_rx) = connect(&test.hub, other, 8);
    barrier(&test.hub, &mut test.events).await;
    test.hub.send_to_user(user, ping("to-second")).await.unwrap();

    // Once disconnected, the user is unknown again.
    test.hub.request_disconnection(user, second).unwrap();
    barrier(&test.hub, &mut test.events).await;
    test.hub.send_to_user(user, ping("too-late")).await.unwrap();
    barrier(&test.hub, &mut test.events).await;

    assert_eq!(
        egress_state(&mut first_rx),
        EgressState {
            pings: vec!["to-first".into()],
            notices: vec![DisconnectReason::Replaced],
            closed: true,
        }
    );
    assert_eq!(
        egress_state(&mut second_rx),
        EgressState {
            pings: vec!["to-second".into()],
            notices: vec![],
            closed: true,
        }
    );
    assert_eq!(egress_state(&mut other_rx), open());
}

#[test(serial = "hub-service")]
async fn send_to_user_prunes_a_dead_or_slow_connection_once() {
    let Some(mut test) = hub().await else {
        return;
    };
    let (gone_user, slow_user) = (Uuid::new_v4(), Uuid::new_v4());

    let (gone, gone_rx) = connect(&test.hub, gone_user, 8);
    let (slow, mut slow_rx) = connect(&test.hub, slow_user, 1);
    barrier(&test.hub, &mut test.events).await;
    drop(gone_rx);

    // Every send finds the problem and queues a prune; only the first one removes the connection.
    for idx in 0..3 {
        test.hub
            .send_to_user(gone_user, ping(format!("g-{idx}")))
            .await
            .unwrap();
        test.hub
            .send_to_user(slow_user, ping(format!("s-{idx}")))
            .await
            .unwrap();
    }
    let seen = barrier(&test.hub, &mut test.events).await;

    assert_eq!(lifecycle_of(&seen, gone_user), [Lifecycle::Disconnected(gone)]);
    assert_eq!(lifecycle_of(&seen, slow_user), [Lifecycle::Disconnected(slow)]);
    // The slow client keeps what fit in its buffer; the notice did not fit.
    assert_eq!(
        egress_state(&mut slow_rx),
        EgressState {
            pings: vec!["s-0".into()],
            notices: vec![],
            closed: true,
        }
    );
    assert_eq!(
        drops(&seen),
        dropped(&[(DisconnectReason::DeadChannel, 1), (DisconnectReason::SlowConsumer, 1)])
    );
    assert_consistent(&test.hub, &test.registry, [gone_user, slow_user], &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn directed_and_broadcast_sends_reach_only_their_targets() {
    let Some(mut test) = hub().await else {
        return;
    };
    let (user, other, hub_only) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());

    let (first, mut first_rx) = connect(&test.hub, user, 8);
    let (second, mut second_rx) = connect(&test.hub, user, 8);
    let (_other, mut other_rx) = connect(&test.hub, other, 8);
    let (_hub_only, mut hub_only_rx) = connect_on(&test.hub, hub_only, 8, vec![TopicKey::Hub]);
    barrier(&test.hub, &mut test.events).await;

    // A replaced connection is unknown: the send reaches nobody, the new connection included.
    assert_eq!(
        test.hub.send_to_connection(first, ping("to-first")).await.ok(),
        Some(false)
    );
    assert_eq!(
        test.hub.send_to_connection(second, ping("to-second")).await.ok(),
        Some(true)
    );
    // A broadcast reaches the Chat topic only.
    test.hub.broadcast(ping("to-all")).await.unwrap();
    barrier(&test.hub, &mut test.events).await;

    assert_eq!(
        egress_state(&mut first_rx),
        closed_with(Some(DisconnectReason::Replaced))
    );
    assert_eq!(
        egress_state(&mut second_rx),
        EgressState {
            pings: vec!["to-second".into(), "to-all".into()],
            ..open()
        }
    );
    assert_eq!(
        egress_state(&mut other_rx),
        EgressState {
            pings: vec!["to-all".into()],
            ..open()
        }
    );
    assert_eq!(egress_state(&mut hub_only_rx).pings, Vec::<String>::new());
}

#[test(serial = "hub-service")]
async fn a_late_subscriber_sees_only_later_events() {
    let Some(mut test) = hub().await else {
        return;
    };
    let user = Uuid::new_v4();

    let (first, _first_rx) = connect(&test.hub, user, 8);
    barrier(&test.hub, &mut test.events).await;
    let mut late = test.hub.subscribe(vec![TopicKey::Hub]).await;
    let (second, _second_rx) = connect(&test.hub, user, 8);
    barrier(&test.hub, &mut test.events).await;

    let late_seen = barrier(&test.hub, &mut late).await;
    assert_eq!(
        lifecycle_of(&late_seen, user),
        [Lifecycle::Disconnected(first), Lifecycle::Connected(second)]
    );
}

#[test(serial = "hub-service")]
async fn shutdown_drops_every_connection_and_publishes_nothing_after() {
    let Some(mut test) = hub().await else {
        return;
    };
    let users: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

    let mut receivers: Vec<_> = users.iter().map(|user| connect(&test.hub, *user, 8).1).collect();
    barrier(&test.hub, &mut test.events).await;

    test.hub.request_shutdown().unwrap();
    // Commands queued behind the Shutdown are never processed.
    let late_user = Uuid::new_v4();
    let _ = test
        .hub
        .request_connection(late_user, key(), tokio::sync::mpsc::channel(1).0, vec![]);

    // The subscribers are released after the Shutdown event, so the stream ends right after it.
    let mut after = vec![];
    while let Some(message) = tokio::time::timeout(BARRIER_TIMEOUT, test.events.recv())
        .await
        .expect("shutdown timed out")
    {
        after.push(message);
    }
    assert!(
        matches!(after.as_slice(), [HubMessage::Hub(HubEvent::Shutdown)]),
        "only the Shutdown event is published: {after:?}"
    );
    wait_stopped(&test.hub).await;

    for rx in &mut receivers {
        assert_eq!(egress_state(rx), closed_with(Some(DisconnectReason::Shutdown)));
    }
    assert_eq!(test.hub.stats().await.subscribers, 0);
    assert_consistent(
        &test.hub,
        &test.registry,
        users.into_iter().chain([late_user]),
        &HashMap::new(),
    )
    .await;
}

#[test(serial = "hub-service")]
async fn registry_change_drops_only_a_connection_the_registry_no_longer_holds() {
    let Some(mut test) = hub().await else {
        return;
    };
    let user = Uuid::new_v4();
    let (connection, mut rx) = connect(&test.hub, user, 8);
    barrier(&test.hub, &mut test.events).await;

    // A change notice while the registry still holds this connection is a no-op.
    test.hub.notify_registry_changed(user).unwrap();
    test.hub.notify_registry_changed(Uuid::new_v4()).unwrap();
    let seen = barrier(&test.hub, &mut test.events).await;
    assert_eq!(lifecycle_of(&seen, user), []);
    assert_eq!(egress_state(&mut rx), open());

    // Another instance took the user's slot: the local connection is dropped, the winner kept.
    let winner = Uuid::new_v4();
    {
        let mut context = test.registry.create_context().await.unwrap();
        context.create_connection(user, winner).await.unwrap();
    }
    test.hub.notify_registry_changed(user).unwrap();
    let seen = barrier(&test.hub, &mut test.events).await;

    assert_eq!(lifecycle_of(&seen, user), [Lifecycle::Disconnected(connection)]);
    assert_eq!(
        egress_state(&mut rx),
        closed_with(Some(DisconnectReason::RegistryInactive))
    );
    assert_eq!(drops(&seen), dropped(&[(DisconnectReason::RegistryInactive, 1)]));
    assert_eq!(find_registry_connection(&test.registry, user).await, Some(winner));
    delete_key(&test.redis, &registry_key(user)).await;
}

#[test(serial = "hub-service")]
async fn heartbeat_refreshes_live_rows_and_drops_expired_ones() {
    let tick = Duration::from_millis(100);
    let Some(mut test) = create_test_hub(TestHubOptions {
        heartbeat: tick,
        ..Default::default()
    })
    .await
    else {
        return;
    };
    let (live_user, expired_user) = (Uuid::new_v4(), Uuid::new_v4());
    let (live, mut live_rx) = connect(&test.hub, live_user, 8);
    let (expired, mut expired_rx) = connect(&test.hub, expired_user, 8);
    barrier(&test.hub, &mut test.events).await;

    // One row about to expire, the other already gone, as after a TTL expiry.
    {
        let mut conn = test.redis.get().await.unwrap();
        let _: () = redis::cmd("EXPIRE")
            .arg(registry_key(live_user))
            .arg(5)
            .query_async(&mut *conn)
            .await
            .unwrap();
    }
    delete_key(&test.redis, &registry_key(expired_user)).await;

    let seen = wait_event(
        &mut test.events,
        BARRIER_TIMEOUT,
        |event| matches!(event, HubEvent::UserDisconnected { connection_id, .. } if *connection_id == expired),
    )
    .await
    .expect("the heartbeat must drop the connection whose row expired");
    assert_eq!(lifecycle_of(&seen, live_user), []);
    tokio::time::sleep(tick * 3).await;
    barrier(&test.hub, &mut test.events).await;

    assert_eq!(
        egress_state(&mut expired_rx),
        closed_with(Some(DisconnectReason::HeartbeatInactive))
    );
    assert_eq!(egress_state(&mut live_rx), open());
    let ttl: i64 = {
        let mut conn = test.redis.get().await.unwrap();
        redis::cmd("TTL")
            .arg(registry_key(live_user))
            .query_async(&mut *conn)
            .await
            .unwrap()
    };
    assert!(ttl > 5, "the heartbeat must refresh the TTL of a live row, got {ttl}");
    assert_consistent(
        &test.hub,
        &test.registry,
        [live_user, expired_user],
        &HashMap::from([(live_user, live)]),
    )
    .await;

    test.hub.request_disconnection(live_user, live).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn session_check_drops_only_connections_with_an_invalid_session() {
    let tick = Duration::from_millis(100);
    let Some(mut test) = create_test_hub(TestHubOptions {
        session_check: tick,
        ..Default::default()
    })
    .await
    else {
        return;
    };
    let (valid_user, invalid_user) = (Uuid::new_v4(), Uuid::new_v4());
    let valid_key = key();
    seed_session(&test.redis, valid_user, valid_key).await;

    let (valid, mut valid_rx) = connect_with_key(&test.hub, valid_user, valid_key, 8);
    let (invalid, mut invalid_rx) = connect(&test.hub, invalid_user, 8);

    let seen = wait_event(
        &mut test.events,
        BARRIER_TIMEOUT,
        |event| matches!(event, HubEvent::UserDisconnected { connection_id, .. } if *connection_id == invalid),
    )
    .await
    .expect("the session check must drop the connection without a session");
    tokio::time::sleep(tick * 3).await;
    let seen = [seen, barrier(&test.hub, &mut test.events).await].concat();

    assert_eq!(lifecycle_of(&seen, valid_user), [Lifecycle::Connected(valid)]);
    assert_eq!(
        egress_state(&mut invalid_rx),
        closed_with(Some(DisconnectReason::SessionInvalid))
    );
    assert_eq!(egress_state(&mut valid_rx), open());
    assert_eq!(drops(&seen), dropped(&[(DisconnectReason::SessionInvalid, 1)]));

    test.hub.request_disconnection(valid_user, valid).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn batched_heartbeat_refreshes_active_and_reports_stale() {
    let Some(mut test) = hub().await else {
        return;
    };
    let (active_a, active_b, never_registered) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let (conn_a, _rx_a) = connect(&test.hub, active_a, 8);
    let (conn_b, _rx_b) = connect(&test.hub, active_b, 8);
    barrier(&test.hub, &mut test.events).await;

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
    barrier(&test.hub, &mut test.events).await;
}
