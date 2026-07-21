use super::HubService;
use crate::{
    models::messages::{HubCommand, HubEvent, HubMessage, TopicKey},
    repositories::hub_registry::{redis::RedisHubConnectionDb, HubConnectionDb, HubRegistry},
};
use ring::rand::SystemRandom;
use shine_infra::{db, session::SessionKey};
use shine_test::test;
use std::{
    collections::HashMap,
    env,
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
    let hub_service = HubService::new(hub_registry.clone());

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
    sender.send_command(HubCommand::DisconnectUser { user_id }).unwrap();
    assert!(
        timeout(Duration::from_millis(120), receiver.recv()).await.is_err(),
        "disconnecting an unknown user should not produce an event"
    );

    sender
        .send_command(HubCommand::ConnectUser {
            user_id,
            session_key: session_key_1,
        })
        .unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, true).await;
    let first_connection_id = find_registry_connection(&hub_registry, user_id)
        .await
        .expect("first connection should exist in registry");

    // Late subscriber should only observe events published after this point.
    let mut late_receiver = hub_service.subscribe(vec![TopicKey::Hub]).await;

    sender
        .send_command(HubCommand::ConnectUser {
            user_id,
            session_key: session_key_2,
        })
        .unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, true).await;
    let second_connection_id = find_registry_connection(&hub_registry, user_id)
        .await
        .expect("reconnected user should remain in registry");
    assert_ne!(
        first_connection_id, second_connection_id,
        "reconnect should replace the old active registry connection"
    );

    sender.send_command(HubCommand::DisconnectUser { user_id }).unwrap();
    sender.send_command(HubCommand::DisconnectUser { user_id }).unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, false).await;

    let events = collect_hub_events(&mut receiver, Duration::from_millis(500)).await;
    let late_events = collect_hub_events(&mut late_receiver, Duration::from_millis(500)).await;

    let connected_keys = events
        .iter()
        .filter_map(|event| match event {
            HubEvent::UserConnected {
                user_id: event_user_id,
                session_key,
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
            |event| matches!(event, HubEvent::UserDisconnected { user_id: event_user_id } if *event_user_id == user_id),
        )
        .count();
    assert!(disconnect_count >= 1, "at least one disconnect event is expected");

    let late_disconnect_count = late_events
        .iter()
        .filter(
            |event| matches!(event, HubEvent::UserDisconnected { user_id: event_user_id } if *event_user_id == user_id),
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
            for idx in 0..8 {
                let key = SessionKey::new_random(&SystemRandom::new()).unwrap();
                task_sender
                    .send_command(HubCommand::ConnectUser { user_id, session_key: key })
                    .unwrap();
                if idx % 2 == 0 {
                    task_sender
                        .send_command(HubCommand::DisconnectUser { user_id })
                        .unwrap();
                }
                tokio::task::yield_now().await;
            }
        }));
    }

    for task in tasks {
        task.await.unwrap();
    }

    for user_id in users {
        sender.send_command(HubCommand::DisconnectUser { user_id }).unwrap();
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
            HubEvent::UserDisconnected { user_id } => {
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
