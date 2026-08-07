use super::{HubIntervals, HubService};
use crate::{
    models::messages::{ChatMessage, HubEvent, HubMessage, TopicKey},
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

    let redis_pool = db::redis::create_redis_pool(redis_cns.as_str()).await.unwrap();
    let hub_registry = RedisHubConnectionDb::new(&redis_pool, 120).await.unwrap();

    // URL_SAFE_NO_PAD: 86 'A' chars decode to 64 zero bytes, a valid cookie key for tests.
    let cookie_secret = "A".repeat(86);
    let session_service = Arc::new(CurrentUserService::new(None, &cookie_secret, "", 120, redis_pool.clone()).unwrap());

    // Long consumer intervals so neither internal loop fires during these tests.
    let intervals = HubIntervals {
        heartbeat: Duration::from_secs(3600),
        session_check: Duration::from_secs(3600),
    };
    let hub_service = HubService::new(hub_registry.clone(), session_service, intervals).await;

    Some((hub_service, hub_registry))
}

/// Connects a user with a live egress channel and returns its connection id alongside the
/// receiver. The receiver must be kept alive: a connection whose channel has closed is pruned the
/// next time the hub broadcasts to it.
async fn connect_live(
    hub_service: &super::HubService,
    user_id: Uuid,
    session_key: SessionKey,
) -> (Uuid, tokio::sync::mpsc::Receiver<HubMessage>) {
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let connection_id = hub_service
        .request_connection(user_id, session_key, tx, vec![TopicKey::Chat, TopicKey::Hub])
        .unwrap();
    (connection_id, rx)
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

async fn collect_hub_events(
    receiver: &mut tokio::sync::mpsc::UnboundedReceiver<HubMessage>,
    window: Duration,
) -> Vec<HubEvent> {
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

    let sender = hub_service.clone();
    let mut receiver = hub_service.subscribe(vec![TopicKey::Hub]).await;

    // Edge case: disconnecting a never-connected user is a no-op.
    sender.request_disconnection(user_id, Uuid::new_v4()).unwrap();
    assert!(
        timeout(Duration::from_millis(120), receiver.recv()).await.is_err(),
        "disconnecting an unknown user should not produce an event"
    );

    let (first_connection_id, _rx_1) = connect_live(&sender, user_id, session_key_1).await;
    wait_for_registry_connection_state(&hub_registry, user_id, true).await;
    assert_eq!(
        find_registry_connection(&hub_registry, user_id).await,
        Some(first_connection_id),
        "hub-issued connection id should be the one stored in the registry"
    );

    // Late subscriber should only observe events published after this point.
    let mut late_receiver = hub_service.subscribe(vec![TopicKey::Hub]).await;

    let (second_connection_id, _rx_2) = connect_live(&sender, user_id, session_key_2).await;
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
    sender.request_disconnection(user_id, first_connection_id).unwrap();
    sender.request_disconnection(user_id, second_connection_id).unwrap();
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

    let sender = hub_service.clone();
    let mut receiver = hub_service.subscribe(vec![TopicKey::Hub]).await;

    let (first_connection_id, _rx_1) = connect_live(&sender, user_id, session_key_1).await;
    wait_for_registry_connection_state(&hub_registry, user_id, true).await;

    // Reconnect WITHOUT an explicit disconnect: the replace itself must emit
    // UserDisconnected for the old connection.
    let (second_connection_id, _rx_2) = connect_live(&sender, user_id, session_key_2).await;
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
    sender.request_disconnection(user_id, second_connection_id).unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, false).await;
}

#[test]
async fn shutdown_publishes_event_and_stops_dispatcher() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };

    let random = SystemRandom::new();
    let user_id = Uuid::new_v4();
    let session_key = SessionKey::new_random(&random).unwrap();

    let sender = hub_service.clone();
    let mut receiver = hub_service.subscribe(vec![TopicKey::Hub]).await;

    // Shut down, then enqueue a connect. The control channel is a single FIFO drained biased-first,
    // so the dispatcher processes Shutdown (and breaks) before ever reaching this connect.
    sender.request_shutdown().unwrap();
    let _ = connect_live(&sender, user_id, session_key).await;

    let events = collect_hub_events(&mut receiver, Duration::from_millis(500)).await;
    assert!(
        events.iter().any(|event| matches!(event, HubEvent::Shutdown)),
        "shutdown should publish a Shutdown event"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, HubEvent::UserConnected { user_id: u, .. } if *u == user_id)),
        "a connect enqueued after shutdown must not be processed"
    );
    assert!(
        find_registry_connection(&hub_registry, user_id).await.is_none(),
        "no registry connection should be created after shutdown"
    );
}

#[test]
async fn chat_payload_is_delivered_and_topic_filtered() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };

    let random = SystemRandom::new();
    let sender = hub_service.clone();
    // A broadcast payload is delivered to addressable connections, topic-filtered: a connection
    // subscribed to Chat receives it; one subscribed to Hub only must not. Inbound payloads go to
    // connections, not to the internal subscriber list.
    let chat_user = Uuid::new_v4();
    let (tx_chat, mut chat_rx) = tokio::sync::mpsc::channel(8);
    let chat_conn = sender
        .request_connection(
            chat_user,
            SessionKey::new_random(&random).unwrap(),
            tx_chat,
            vec![TopicKey::Chat],
        )
        .unwrap();
    wait_for_registry_connection_state(&hub_registry, chat_user, true).await;

    let hub_user = Uuid::new_v4();
    let (tx_hub, mut hub_rx) = tokio::sync::mpsc::channel(8);
    let hub_conn = sender
        .request_connection(
            hub_user,
            SessionKey::new_random(&random).unwrap(),
            tx_hub,
            vec![TopicKey::Hub],
        )
        .unwrap();
    wait_for_registry_connection_state(&hub_registry, hub_user, true).await;

    let author = Uuid::new_v4();
    sender
        .broadcast(HubMessage::Chat(ChatMessage {
            user_id: author,
            text: "hello".to_string(),
        }))
        .await
        .unwrap();

    // The Chat connection receives the payload (skip any lifecycle events that arrive first).
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut delivered = None;
    while Instant::now() < deadline {
        match timeout(deadline.saturating_duration_since(Instant::now()), chat_rx.recv()).await {
            Ok(Some(HubMessage::Chat(message))) => {
                delivered = Some(message);
                break;
            }
            Ok(Some(_)) => {}
            _ => break,
        }
    }
    let message = delivered.expect("chat connection should receive the chat payload");
    assert_eq!(message.user_id, author, "delivered chat should carry the author id");
    assert_eq!(
        message.text, "hello",
        "delivered chat should carry the payload verbatim"
    );

    // The Hub-only connection must never receive a Chat payload (lifecycle events are fine).
    let mut saw_chat = false;
    while let Ok(Some(msg)) = timeout(Duration::from_millis(150), hub_rx.recv()).await {
        if matches!(msg, HubMessage::Chat(_)) {
            saw_chat = true;
        }
    }
    assert!(!saw_chat, "a Hub-only connection must not receive Chat payloads");

    sender.request_disconnection(chat_user, chat_conn).unwrap();
    sender.request_disconnection(hub_user, hub_conn).unwrap();
    wait_for_registry_connection_state(&hub_registry, chat_user, false).await;
    wait_for_registry_connection_state(&hub_registry, hub_user, false).await;
}

#[test]
async fn direct_send_to_connection_moves_to_that_connection_only() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };
    let random = SystemRandom::new();
    let sender = hub_service.clone();

    // Two connected users, each with its own egress channel handed to the hub.
    let user_a = Uuid::new_v4();
    let (tx_a, mut rx_a) = tokio::sync::mpsc::channel(8);
    let conn_a = sender
        .request_connection(
            user_a,
            SessionKey::new_random(&random).unwrap(),
            tx_a,
            vec![TopicKey::Chat, TopicKey::Hub],
        )
        .unwrap();
    wait_for_registry_connection_state(&hub_registry, user_a, true).await;

    let user_b = Uuid::new_v4();
    let (tx_b, mut rx_b) = tokio::sync::mpsc::channel(8);
    let conn_b = sender
        .request_connection(
            user_b,
            SessionKey::new_random(&random).unwrap(),
            tx_b,
            vec![TopicKey::Chat, TopicKey::Hub],
        )
        .unwrap();
    wait_for_registry_connection_state(&hub_registry, user_b, true).await;

    // Directed send to conn_a: only A's channel receives it.
    sender
        .send_to_connection(
            conn_a,
            HubMessage::Chat(ChatMessage {
                user_id: user_a,
                text: "just-a".into(),
            }),
        )
        .await
        .unwrap();

    let got_a = timeout(Duration::from_millis(500), rx_a.recv()).await;
    assert!(matches!(got_a, Ok(Some(HubMessage::Chat(m))) if m.text == "just-a"));

    // B must not receive the directed message (allow lifecycle events through, then assert no chat).
    let mut saw_chat_on_b = false;
    while let Ok(Some(msg)) = timeout(Duration::from_millis(150), rx_b.recv()).await {
        if matches!(msg, HubMessage::Chat(m) if m.text == "just-a") {
            saw_chat_on_b = true;
        }
    }
    assert!(!saw_chat_on_b, "directed send must not reach other connections");

    sender.request_disconnection(user_a, conn_a).unwrap();
    sender.request_disconnection(user_b, conn_b).unwrap();
    wait_for_registry_connection_state(&hub_registry, user_a, false).await;
    wait_for_registry_connection_state(&hub_registry, user_b, false).await;
}

#[test]
async fn registry_change_disconnects_stale_local_connection() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };

    let random = SystemRandom::new();
    let user_id = Uuid::new_v4();
    let session_key = SessionKey::new_random(&random).unwrap();

    let sender = hub_service.clone();
    let mut receiver = hub_service.subscribe(vec![TopicKey::Hub]).await;

    let (local_connection_id, _rx) = connect_live(&sender, user_id, session_key).await;
    wait_for_registry_connection_state(&hub_registry, user_id, true).await;

    // Simulate another service instance taking over the user's single connection slot: overwrite
    // the registry key with a different connection id, then deliver the registry-change notice.
    let other_instance_connection_id = Uuid::new_v4();
    {
        let mut context = hub_registry.create_context().await.unwrap();
        context
            .create_connection(user_id, other_instance_connection_id)
            .await
            .unwrap();
    }
    sender.notify_registry_changed(user_id).unwrap();

    // This instance must find its local connection is no longer the active one and disconnect it,
    // publishing UserDisconnected for exactly the local (now stale) connection id.
    let events = collect_hub_events(&mut receiver, Duration::from_millis(500)).await;
    assert!(
        events.iter().any(|event| {
            matches!(event, HubEvent::UserDisconnected { user_id: u, connection_id }
                if *u == user_id && *connection_id == local_connection_id)
        }),
        "a registry change that replaced this instance's connection must disconnect it locally"
    );

    // The other instance's entry must be left intact — reconciliation only removes the local stale
    // connection, never the fresher winner.
    assert_eq!(
        find_registry_connection(&hub_registry, user_id).await,
        Some(other_instance_connection_id),
        "reconciliation must not remove the connection owned by the other instance"
    );

    // cleanup
    {
        let mut context = hub_registry.create_context().await.unwrap();
        context
            .remove_connection_if_active(user_id, other_instance_connection_id)
            .await
            .unwrap();
    }
    wait_for_registry_connection_state(&hub_registry, user_id, false).await;
}

#[test]
async fn batched_heartbeat_refreshes_active_and_reports_stale() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };

    let random = SystemRandom::new();
    let sender = hub_service.clone();

    // Two active connections and one that was never registered.
    let active_a = Uuid::new_v4();
    let active_b = Uuid::new_v4();
    let never_registered = Uuid::new_v4();

    let (conn_a, _rx_a) = connect_live(&sender, active_a, SessionKey::new_random(&random).unwrap()).await;
    let (conn_b, _rx_b) = connect_live(&sender, active_b, SessionKey::new_random(&random).unwrap()).await;
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

    // Single-heartbeat CAS: the matching id is still active (TTL refreshed, not removed), while a
    // stale id for the same user reports inactive.
    assert!(
        hub_service
            .heartbeat_registry_connection(active_a, conn_a)
            .await
            .unwrap(),
        "the active connection should remain active after a batched heartbeat"
    );
    assert!(
        !hub_service
            .heartbeat_registry_connection(active_a, Uuid::new_v4())
            .await
            .unwrap(),
        "a stale connection id should report inactive"
    );

    // cleanup
    sender.request_disconnection(active_a, conn_a).unwrap();
    sender.request_disconnection(active_b, conn_b).unwrap();
    wait_for_registry_connection_state(&hub_registry, active_a, false).await;
    wait_for_registry_connection_state(&hub_registry, active_b, false).await;
}

#[test]
async fn slow_connection_is_auto_dropped_when_it_falls_behind() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };

    let random = SystemRandom::new();
    let sender = hub_service.clone();

    // Let the internal consumers (heartbeat, session checker) finish subscribing so the connection
    // count is stable before we add ours.
    sleep(Duration::from_millis(100)).await;
    let baseline_connections = hub_service.stats().await.connections;

    // A slow client: a capacity-1 connection whose egress channel is never drained.
    let user_id = Uuid::new_v4();
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    let connection_id = sender
        .request_connection(
            user_id,
            SessionKey::new_random(&random).unwrap(),
            tx,
            vec![TopicKey::Chat],
        )
        .unwrap();
    wait_for_registry_connection_state(&hub_registry, user_id, true).await;
    assert_eq!(
        hub_service.stats().await.connections,
        baseline_connections + 1,
        "the slow connection should be registered"
    );

    // Flood the Chat topic without ever draining `_rx`. The first broadcast fills the capacity-1
    // buffer; the next finds it full, marks the connection dead, and enqueues DropConnection.
    let chatter = Uuid::new_v4();
    for idx in 0..32 {
        sender
            .broadcast(HubMessage::Chat(ChatMessage {
                user_id: chatter,
                text: format!("msg {idx}"),
            }))
            .await
            .unwrap();
    }

    // The hub must prune the lagging connection: its registry entry is removed and the local
    // connection count returns to baseline.
    wait_for_registry_connection_state(&hub_registry, user_id, false).await;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if hub_service.stats().await.connections == baseline_connections {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "lagging connection {connection_id} was not removed from the hub connection set"
        );
        sleep(Duration::from_millis(20)).await;
    }
}

#[test]
async fn concurrent_connect_disconnect_churn_keeps_consistent_registry_state() {
    let (hub_service, hub_registry) = match create_test_hub_service().await {
        Some(data) => data,
        None => return,
    };

    let users = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];

    let sender = hub_service.clone();
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
            // Track the last connection id created so cleanup can target the active one. Keep the
            // egress receivers alive for the task's duration so broadcasts don't prune connections
            // out from under the churn (a closed channel would inject extra disconnect events).
            let mut last_connection_id = Uuid::new_v4();
            let mut receivers = vec![];
            for idx in 0..8 {
                let key = SessionKey::new_random(&SystemRandom::new()).unwrap();
                let (tx, rx) = tokio::sync::mpsc::channel(64);
                let connection_id = task_sender
                    .request_connection(user_id, key, tx, vec![TopicKey::Chat, TopicKey::Hub])
                    .unwrap();
                receivers.push(rx);
                last_connection_id = connection_id;
                if idx % 2 == 0 {
                    task_sender.request_disconnection(user_id, connection_id).unwrap();
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
        sender
            .request_disconnection(user_id, last_connection_ids[&user_id])
            .unwrap();
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
