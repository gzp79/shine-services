use super::{HubIntervals, HubService};
use crate::{
    models::messages::{HubEvent, HubMessage, TopicKey},
    repositories::hub_registry::{redis::RedisHubConnectionDb, HubConnection, HubConnectionDb, HubRegistry},
};
use ring::rand::SystemRandom;
use shine_infra::{
    db,
    session::{CurrentUserService, SessionKey},
};
use shine_test::test;
use std::{
    collections::HashMap,
    env,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::watch,
    time::{sleep, timeout},
};
use uuid::Uuid;

async fn create_test_hub_service() -> Option<(HubService, RedisHubConnectionDb)> {
    let redis_cns = env::var("SHINE_TEST_REDIS_CNS")
        .or_else(|_| env::var("SHINE_REDIS_CNS"))
        .ok();

    let Some(redis_cns) = redis_cns else {
        log::warn!("Missing SHINE_TEST_REDIS_CNS/SHINE_REDIS_CNS, skipping test");
        return None;
    };

    let redis_pool = db::create_redis_pool(redis_cns.as_str()).await.unwrap();
    let hub_registry = RedisHubConnectionDb::new(&redis_pool, 120).await.unwrap();

    // URL_SAFE_NO_PAD: 86 'A' chars decode to 64 zero bytes, a valid cookie key for tests.
    let cookie_secret = "A".repeat(86);
    let session_service = Arc::new(CurrentUserService::new(None, &cookie_secret, "", 120, redis_pool.clone()).unwrap());

    // Long consumer intervals so neither internal loop fires during these tests.
    let intervals = HubIntervals {
        heartbeat: Duration::from_secs(3600),
        session_check: Duration::from_secs(3600),
    };
    let hub_service = HubService::new(hub_registry.clone(), session_service, intervals);

    Some((hub_service, hub_registry))
}

async fn find_registry_connection(hub_registry: &RedisHubConnectionDb, user_id: Uuid) -> Option<Uuid> {
    let mut context = hub_registry.create_context().await.unwrap();
    context
        .find_connection_by_user(user_id)
        .await
        .unwrap()
        .map(|connection| connection.connection_id)
}

async fn wait_for_registry_connection_state(hub_registry: &RedisHubConnectionDb, user_id: Uuid, should_exist: bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let exists = find_registry_connection(hub_registry, user_id).await.is_some();
        if exists == should_exist {
            return;
        }
        if Instant::now() >= deadline {
            panic!(
                "Timed out waiting for registry connection state for user {user_id}, expected exists={should_exist}"
            );
        }
        sleep(Duration::from_millis(20)).await;
    }
}

async fn collect_hub_events(receiver: &mut crate::services::HubReceiver, window: Duration) -> Vec<HubEvent> {
    let deadline = Instant::now() + window;
    let mut events = vec![];

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match timeout(remaining, receiver.recv()).await {
            Ok(Some(HubMessage::Hub(event))) => events.push(event),
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => break,
        }
    }

    events
}

#[test]
async fn reconnect_and_idempotent_disconnect_edge_cases() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };

    let random = SystemRandom::new();
    let user_id = Uuid::new_v4();
    let session_key_1 = SessionKey::new_random(&random).unwrap();
    let session_key_2 = SessionKey::new_random(&random).unwrap();

    let sender = hub_service.sender();
    let mut receiver = hub_service.subscribe(vec![TopicKey::Hub]).await;

    // Edge case: disconnecting a never-connected user is a no-op.
    sender.disconnect(user_id, Uuid::new_v4()).unwrap();
    assert!(
        timeout(Duration::from_millis(120), receiver.recv()).await.is_err(),
        "disconnecting an unknown user should not produce an event"
    );

    let first_connection_id = sender.connect(user_id, session_key_1).unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, true).await;
    assert_eq!(
        find_registry_connection(&hub_registry, user_id).await,
        Some(first_connection_id),
        "hub-issued connection id should be the one stored in the registry"
    );

    // Late subscriber should only observe events published after this point.
    let mut late_receiver = hub_service.subscribe(vec![TopicKey::Hub]).await;

    let second_connection_id = sender.connect(user_id, session_key_2).unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, true).await;
    assert_eq!(
        find_registry_connection(&hub_registry, user_id).await,
        Some(second_connection_id),
        "reconnect should replace the old active registry connection"
    );
    assert_ne!(
        first_connection_id, second_connection_id,
        "reconnect should mint a distinct connection id"
    );

    // Edge case: a stale disconnect for the replaced connection is ignored; only the
    // current connection can be disconnected.
    sender.disconnect(user_id, first_connection_id).unwrap();
    sender.disconnect(user_id, second_connection_id).unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, false).await;

    let events = collect_hub_events(&mut receiver, Duration::from_millis(500)).await;
    let late_events = collect_hub_events(&mut late_receiver, Duration::from_millis(500)).await;

    let connected_keys = events
        .iter()
        .filter_map(|event| match event {
            HubEvent::UserConnected {
                user_id: event_user_id,
                session_key,
                ..
            } if *event_user_id == user_id => Some(*session_key),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        connected_keys.contains(&session_key_1),
        "first connect event should be observable"
    );
    assert!(
        connected_keys.contains(&session_key_2),
        "reconnect should publish the new session key"
    );

    let late_connected_keys = late_events
        .iter()
        .filter_map(|event| match event {
            HubEvent::UserConnected {
                user_id: event_user_id,
                session_key,
                ..
            } if *event_user_id == user_id => Some(*session_key),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        !late_connected_keys.contains(&session_key_1),
        "late subscriber must not receive historical first-connect event"
    );
    assert!(
        late_connected_keys.contains(&session_key_2),
        "late subscriber should receive reconnect event"
    );

    let disconnect_count = events
        .iter()
        .filter(
            |event| matches!(event, HubEvent::UserDisconnected { user_id: event_user_id, .. } if *event_user_id == user_id),
        )
        .count();
    assert!(disconnect_count >= 1, "at least one disconnect event is expected");

    let late_disconnect_count = late_events
        .iter()
        .filter(
            |event| matches!(event, HubEvent::UserDisconnected { user_id: event_user_id, .. } if *event_user_id == user_id),
        )
        .count();
    assert!(
        late_disconnect_count >= 1,
        "late subscriber should receive disconnect event after reconnect"
    );

    assert!(
        find_registry_connection(&hub_registry, user_id).await.is_none(),
        "registry connection should be removed after disconnect"
    );
}

#[test]
async fn reconnect_replace_emits_disconnect_for_old_connection() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };

    let random = SystemRandom::new();
    let user_id = Uuid::new_v4();
    let session_key_1 = SessionKey::new_random(&random).unwrap();
    let session_key_2 = SessionKey::new_random(&random).unwrap();

    let sender = hub_service.sender();
    let mut receiver = hub_service.subscribe(vec![TopicKey::Hub]).await;

    let first_connection_id = sender.connect(user_id, session_key_1).unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, true).await;

    // Reconnect WITHOUT an explicit disconnect: the replace itself must emit
    // UserDisconnected for the old connection.
    let second_connection_id = sender.connect(user_id, session_key_2).unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, true).await;

    let events = collect_hub_events(&mut receiver, Duration::from_millis(500)).await;

    let disconnected_old = events.iter().any(|event| {
        matches!(event, HubEvent::UserDisconnected { user_id: u, connection_id }
            if *u == user_id && *connection_id == first_connection_id)
    });
    assert!(
        disconnected_old,
        "replacing a connection must publish UserDisconnected for the old connection id"
    );
    assert_ne!(first_connection_id, second_connection_id);

    // cleanup
    sender.disconnect(user_id, second_connection_id).unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, false).await;
}

#[test]
async fn heartbeat_registry_connection_matches_active_only() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };

    let random = SystemRandom::new();
    let user_id = Uuid::new_v4();
    let session_key = SessionKey::new_random(&random).unwrap();

    let sender = hub_service.sender();
    let connection_id = sender.connect(user_id, session_key).unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, true).await;

    assert_eq!(
        hub_service
            .heartbeat_registry_connection(user_id, connection_id)
            .await
            .unwrap(),
        true,
        "heartbeat of the active connection should succeed"
    );
    assert_eq!(
        hub_service
            .heartbeat_registry_connection(user_id, Uuid::new_v4())
            .await
            .unwrap(),
        false,
        "heartbeat of a stale connection id should report inactive"
    );

    // cleanup
    sender.disconnect(user_id, connection_id).unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, false).await;
}

#[test]
async fn batched_heartbeat_refreshes_active_and_reports_stale() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };

    let random = SystemRandom::new();
    let sender = hub_service.sender();

    // Two active connections and one that was never registered.
    let active_a = Uuid::new_v4();
    let active_b = Uuid::new_v4();
    let never_registered = Uuid::new_v4();

    let conn_a = sender
        .connect(active_a, SessionKey::new_random(&random).unwrap())
        .unwrap();
    let conn_b = sender
        .connect(active_b, SessionKey::new_random(&random).unwrap())
        .unwrap();
    wait_for_registry_connection_state(&hub_registry, active_a, true).await;
    wait_for_registry_connection_state(&hub_registry, active_b, true).await;

    // Batch mixes: the two active ids, a stale id for an active user, and a user with no entry.
    let requested = vec![
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

    let mut stale = hub_service.heartbeat_registry_connections(&requested).await.unwrap();
    stale.sort_by_key(|c| c.user_id);
    let mut expected = vec![
        HubConnection {
            user_id: active_b,
            connection_id: requested[1].connection_id,
        },
        HubConnection {
            user_id: never_registered,
            connection_id: requested[2].connection_id,
        },
    ];
    expected.sort_by_key(|c| c.user_id);
    assert_eq!(
        stale, expected,
        "only the mismatching / missing entries are reported stale"
    );

    // The matching entry is still active after the batched heartbeat (TTL refreshed, not removed).
    assert_eq!(
        hub_service
            .heartbeat_registry_connection(active_a, conn_a)
            .await
            .unwrap(),
        true,
        "the active connection should remain active after a batched heartbeat"
    );

    // cleanup
    sender.disconnect(active_a, conn_a).unwrap();
    sender.disconnect(active_b, conn_b).unwrap();
    wait_for_registry_connection_state(&hub_registry, active_a, false).await;
    wait_for_registry_connection_state(&hub_registry, active_b, false).await;
}

#[test]
async fn concurrent_connect_disconnect_churn_keeps_consistent_registry_state() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };

    let users = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];

    let sender = hub_service.sender();
    let mut receiver = hub_service.subscribe(vec![TopicKey::Hub]).await;

    // Drain events while churn is running to avoid artificial subscriber backpressure in test code.
    let (stop_tx, mut stop_rx) = watch::channel(false);
    let collector = tokio::spawn(async move {
        let mut events = Vec::<HubEvent>::new();
        loop {
            tokio::select! {
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        break;
                    }
                }
                message = receiver.recv() => {
                    match message {
                        Some(HubMessage::Hub(event)) => events.push(event),
                        Some(_) => {}
                        None => break,
                    }
                }
            }
        }
        events
    });

    let mut tasks = vec![];
    for user_id in users {
        let task_sender = sender.clone();
        tasks.push(tokio::spawn(async move {
            // Track the last connection id created so cleanup can target the active one.
            let mut last_connection_id = Uuid::new_v4();
            for idx in 0..8 {
                let key = SessionKey::new_random(&SystemRandom::new()).unwrap();
                let connection_id = task_sender.connect(user_id, key).unwrap();
                last_connection_id = connection_id;
                if idx % 2 == 0 {
                    task_sender.disconnect(user_id, connection_id).unwrap();
                }
                tokio::task::yield_now().await;
            }
            (user_id, last_connection_id)
        }));
    }

    let mut last_connection_ids = HashMap::<Uuid, Uuid>::new();
    for task in tasks {
        let (user_id, connection_id) = task.await.unwrap();
        last_connection_ids.insert(user_id, connection_id);
    }

    for user_id in users {
        sender.disconnect(user_id, last_connection_ids[&user_id]).unwrap();
        wait_for_registry_connection_state(&hub_registry, user_id, false).await;
    }

    sleep(Duration::from_millis(120)).await;
    let _ = stop_tx.send(true);
    let events = collector.await.unwrap();

    let mut connected_count_by_user = HashMap::<Uuid, usize>::new();
    let mut disconnected_count_by_user = HashMap::<Uuid, usize>::new();

    for event in &events {
        match event {
            HubEvent::UserConnected { user_id, .. } => {
                *connected_count_by_user.entry(*user_id).or_insert(0) += 1;
            }
            HubEvent::UserDisconnected { user_id, .. } => {
                *disconnected_count_by_user.entry(*user_id).or_insert(0) += 1;
            }
            HubEvent::Shutdown => {}
        }
    }

    for user_id in users {
        let connected = connected_count_by_user.get(&user_id).copied().unwrap_or(0);
        let disconnected = disconnected_count_by_user.get(&user_id).copied().unwrap_or(0);
        assert!(connected > 0, "user {user_id} should have at least one connect event");
        assert!(
            disconnected <= connected,
            "disconnect events cannot outnumber connect events for user {user_id}"
        );
        assert!(
            find_registry_connection(&hub_registry, user_id).await.is_none(),
            "user {user_id} should not remain connected after final disconnect"
        );
    }

    let total_connected = connected_count_by_user.values().sum::<usize>();
    let total_disconnected = disconnected_count_by_user.values().sum::<usize>();
    assert!(
        total_connected >= users.len(),
        "expected at least one connect event per user"
    );
    assert!(
        total_disconnected > 0,
        "expected disconnect events under concurrent churn"
    );
}
