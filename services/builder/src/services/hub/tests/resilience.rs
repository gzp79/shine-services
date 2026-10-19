//! The hub staying consistent when its surroundings misbehave: missed registry notices, reconnect
//! floods and a shutdown of many connections.

use crate::{
    models::{
        messages::{DisconnectReason, HubEvent},
        HubError,
    },
    repositories::hub_registry::{HubConnectionDb, HubRegistry},
    services::hub_test_utils::{
        assert_consistent, barrier, closed_with, connect, create_test_hub, delete_key, dropped, drops, egress_state,
        find_registry_connection, lifecycle_of, open, registry_key, wait_event, wait_stopped, Lifecycle, TestHub,
        TestHubOptions, BARRIER_TIMEOUT,
    },
};
use shine_test::test;
use std::{collections::HashMap, time::Duration};
use uuid::Uuid;

async fn hub() -> Option<TestHub> {
    create_test_hub(TestHubOptions::default()).await
}

#[test(serial = "hub-service")]
async fn a_resync_drops_only_the_connections_another_instance_took_over() {
    let Some(mut test) = hub().await else {
        return;
    };
    let (kept_user, lost_user, restored_user) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let (kept, mut kept_rx) = connect(&test.hub, kept_user, 8);
    let (lost, mut lost_rx) = connect(&test.hub, lost_user, 8);
    let (restored, mut restored_rx) = connect(&test.hub, restored_user, 8);
    barrier(&test.hub, &mut test.events).await;

    // The takeover notice was missed (a pub/sub outage): the registry changed without the hub hearing.
    let winner = Uuid::new_v4();
    {
        let mut context = test.registry.create_context().await.unwrap();
        context.create_connection(lost_user, winner).await.unwrap();
    }
    delete_key(&test.redis, &registry_key(restored_user)).await;

    test.hub.request_registry_resync().unwrap();
    let seen = barrier(&test.hub, &mut test.events).await;

    assert_eq!(lifecycle_of(&seen, lost_user), [Lifecycle::Disconnected(lost)]);
    assert_eq!(lifecycle_of(&seen, kept_user), []);
    assert_eq!(lifecycle_of(&seen, restored_user), []);
    assert_eq!(
        egress_state(&mut lost_rx),
        closed_with(Some(DisconnectReason::RegistryInactive))
    );
    assert_eq!(egress_state(&mut kept_rx), open());
    assert_eq!(egress_state(&mut restored_rx), open());
    assert_eq!(drops(&seen), dropped(&[(DisconnectReason::RegistryInactive, 1)]));
    assert_eq!(find_registry_connection(&test.registry, lost_user).await, Some(winner));
    assert_eq!(
        find_registry_connection(&test.registry, restored_user).await,
        Some(restored),
        "a missing entry has no competing owner and is restored"
    );

    test.hub.request_disconnection(kept_user, kept).unwrap();
    test.hub.request_disconnection(restored_user, restored).unwrap();
    barrier(&test.hub, &mut test.events).await;
    delete_key(&test.redis, &registry_key(lost_user)).await;
}

#[test(serial = "hub-service")]
async fn a_resync_without_connections_is_a_noop() {
    let Some(mut test) = hub().await else {
        return;
    };
    test.hub.request_registry_resync().unwrap();
    let seen = barrier(&test.hub, &mut test.events).await;
    assert!(seen.is_empty(), "{seen:?}");
}

#[test(serial = "hub-service")]
async fn a_user_reconnecting_too_fast_is_refused_before_the_upgrade() {
    let Some(test) = hub().await else {
        return;
    };
    let (user, other) = (Uuid::new_v4(), Uuid::new_v4());

    assert!(test.hub.check_connect_allowed(user).is_ok());
    assert!(matches!(
        test.hub.check_connect_allowed(user),
        Err(HubError::TooManyConnects)
    ));
    // Other users are not affected, and the user may try again once the interval has passed.
    assert!(test.hub.check_connect_allowed(other).is_ok());
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(test.hub.check_connect_allowed(user).is_ok());
}

#[test(serial = "hub-service")]
async fn the_status_reports_a_running_hub_and_an_empty_queue_once_it_has_caught_up() {
    let Some(mut test) = hub().await else {
        return;
    };
    let users: Vec<_> = (0..20).map(|_| Uuid::new_v4()).collect();
    let _receivers: Vec<_> = users.iter().map(|user| connect(&test.hub, *user, 8)).collect();
    barrier(&test.hub, &mut test.events).await;

    let stats = test.hub.stats().await;
    assert!(stats.running);
    assert_eq!(stats.connections, 20);
    assert_eq!(stats.pending_commands, 0);
    assert_eq!(stats.stopped_tasks, Vec::<&str>::new());

    test.hub.request_shutdown().unwrap();
    test.hub.wait_stopped().await;
    let stats = test.hub.stats().await;
    assert!(!stats.running);
    assert_eq!(stats.connections, 0);
}

#[test(serial = "hub-service")]
async fn a_shutdown_of_many_connections_closes_every_socket_and_clears_the_registry() {
    let Some(mut test) = hub().await else {
        return;
    };
    let users: Vec<Uuid> = (0..300).map(|_| Uuid::new_v4()).collect();
    let mut receivers: Vec<_> = users.iter().map(|user| connect(&test.hub, *user, 8).1).collect();
    barrier(&test.hub, &mut test.events).await;
    assert_eq!(test.hub.stats().await.connections, users.len());

    test.hub.request_shutdown().unwrap();
    tokio::time::timeout(BARRIER_TIMEOUT, test.hub.wait_stopped())
        .await
        .expect("the shutdown did not finish");
    wait_stopped(&test.hub).await;

    for rx in &mut receivers {
        assert_eq!(egress_state(rx), closed_with(Some(DisconnectReason::Shutdown)));
    }
    assert_consistent(&test.hub, &test.registry, users, &HashMap::new()).await;
}

#[test(serial = "hub-service")]
async fn a_connect_after_the_registry_listener_started_is_seen_by_the_other_instance() {
    // The listener is subscribed by the time `new` returns, so a takeover right after startup is not missed.
    let (Some(mut a), Some(b)) = (hub().await, hub().await) else {
        return;
    };
    let user = Uuid::new_v4();
    let (on_a, mut a_rx) = connect(&a.hub, user, 8);
    barrier(&a.hub, &mut a.events).await;

    let (_on_b, _b_rx) = connect(&b.hub, user, 8);
    wait_event(
        &mut a.events,
        BARRIER_TIMEOUT,
        |event| matches!(event, HubEvent::UserDisconnected { connection_id, .. } if *connection_id == on_a),
    )
    .await
    .expect("the takeover was not noticed");
    assert_eq!(
        egress_state(&mut a_rx),
        closed_with(Some(DisconnectReason::RegistryInactive))
    );
    delete_key(&b.redis, &registry_key(user)).await;
}
