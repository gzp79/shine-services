//! Unit tests of [`Connections`]: send outcomes, topic filter, CAS removal, drop notice.

use crate::{
    models::messages::{DisconnectReason, HubEvent, HubMessage, TopicKey},
    services::{
        hub::hub_connections::{Connections, DeadConnection},
        hub_test_utils::ping,
    },
};
use ring::rand::SystemRandom;
use shine_infra::session::SessionKey;
use shine_test::test;
use tokio::sync::mpsc;
use uuid::Uuid;

fn disconnected(user_id: Uuid, connection_id: Uuid, reason: DisconnectReason) -> HubEvent {
    HubEvent::UserDisconnected { user_id, connection_id, reason }
}

fn key() -> SessionKey {
    SessionKey::new_random(&SystemRandom::new()).unwrap()
}

#[test]
async fn directed_send_moves_to_the_one_connection() {
    let users = Connections::new();
    let (tx, mut rx) = mpsc::channel(4);
    let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
    users
        .register_connection(user_id, conn_id, key(), tx, vec![TopicKey::Ping])
        .await;

    assert!(users.send_to_connection(conn_id, ping("hi")).await.is_ok());
    assert!(users.send_to_user(user_id, ping("yo")).await.is_ok());

    assert!(matches!(rx.recv().await, Some(HubMessage::Ping(m)) if m.id == "hi"));
    assert!(matches!(rx.recv().await, Some(HubMessage::Ping(m)) if m.id == "yo"));
}

#[test]
async fn directed_send_to_unknown_target_is_ok_noop() {
    let users = Connections::new();
    assert_eq!(users.send_to_connection(Uuid::new_v4(), ping("x")).await, Ok(false));
    assert!(users.send_to_user(Uuid::new_v4(), ping("x")).await.is_ok());
}

#[test]
async fn send_to_user_follows_the_active_connection_and_reports_dead_ones() {
    let users = Connections::new();
    let user_id = Uuid::new_v4();
    let (old_tx, mut old_rx) = mpsc::channel(4);
    let (new_tx, mut new_rx) = mpsc::channel(1);
    let (old_id, new_id) = (Uuid::new_v4(), Uuid::new_v4());
    users.register_connection(user_id, old_id, key(), old_tx, vec![]).await;
    users.register_connection(user_id, new_id, key(), new_tx, vec![]).await;

    // The user's latest registration is its active connection.
    assert!(users.send_to_user(user_id, ping("1")).await.is_ok());
    assert!(matches!(new_rx.try_recv(), Ok(HubMessage::Ping(m)) if m.id == "1"));
    assert!(old_rx.try_recv().is_err());

    assert!(users.send_to_user(user_id, ping("2")).await.is_ok());
    assert_eq!(
        users.send_to_user(user_id, ping("3")).await,
        Err(DeadConnection {
            connection_id: new_id,
            reason: DisconnectReason::SlowConsumer
        })
    );
    drop(new_rx);
    assert_eq!(
        users.send_to_user(user_id, ping("4")).await,
        Err(DeadConnection {
            connection_id: new_id,
            reason: DisconnectReason::DeadChannel
        })
    );
}

#[test]
async fn full_channel_reports_dead_connection() {
    let users = Connections::new();
    let (tx, _rx) = mpsc::channel(1);
    let conn_id = Uuid::new_v4();
    users
        .register_connection(Uuid::new_v4(), conn_id, key(), tx, vec![TopicKey::Ping])
        .await;

    assert!(users.send_to_connection(conn_id, ping("1")).await.is_ok()); // fills capacity 1
    assert_eq!(
        users.send_to_connection(conn_id, ping("2")).await,
        Err(DeadConnection {
            connection_id: conn_id,
            reason: DisconnectReason::SlowConsumer
        })
    );
}

#[test]
async fn closed_channel_reports_dead_connection() {
    let users = Connections::new();
    let (tx, rx) = mpsc::channel(1);
    let conn_id = Uuid::new_v4();
    users
        .register_connection(Uuid::new_v4(), conn_id, key(), tx, vec![TopicKey::Ping])
        .await;
    drop(rx);
    assert_eq!(
        users.send_to_connection(conn_id, ping("1")).await,
        Err(DeadConnection {
            connection_id: conn_id,
            reason: DisconnectReason::DeadChannel
        })
    );
}

#[test]
async fn broadcast_clones_to_matching_topic_only() {
    let users = Connections::new();
    let (tx_chat, mut rx_chat) = mpsc::channel(4);
    let (tx_hub, mut rx_hub) = mpsc::channel(4);
    let chat_conn = Uuid::new_v4();
    let hub_conn = Uuid::new_v4();
    users
        .register_connection(Uuid::new_v4(), chat_conn, key(), tx_chat, vec![TopicKey::Ping])
        .await;
    users
        .register_connection(Uuid::new_v4(), hub_conn, key(), tx_hub, vec![TopicKey::Hub])
        .await;

    assert!(users.broadcast(ping("broadcast")).await.is_ok());
    assert!(matches!(rx_chat.recv().await, Some(HubMessage::Ping(_))));
    // Hub-topic connection must not receive a Chat broadcast.
    assert!(rx_hub.try_recv().is_err());
}

#[test]
async fn removed_connection_releases_its_channel_after_the_notice() {
    let users = Connections::new();
    let (tx_a, mut rx_a) = mpsc::channel(4);
    let (tx_b, mut rx_b) = mpsc::channel(4);
    let (user_a, conn_a) = (Uuid::new_v4(), Uuid::new_v4());
    users.register_connection(user_a, conn_a, key(), tx_a, vec![]).await;
    users
        .register_connection(Uuid::new_v4(), Uuid::new_v4(), key(), tx_b, vec![])
        .await;

    let mut removed = users.remove_all().await;
    assert_eq!(removed.len(), 2);
    assert_eq!(users.len().await, 0);
    removed.sort_by_key(|r| r.connection_id != conn_a);
    let mut removed = removed.into_iter();
    let first = removed.next().unwrap();
    assert_eq!((first.user_id, first.connection_id), (user_a, conn_a));
    first.notify(disconnected(user_a, conn_a, DisconnectReason::Shutdown));
    drop(removed);

    assert!(matches!(
        rx_a.recv().await,
        Some(HubMessage::Hub(HubEvent::UserDisconnected {
            reason: DisconnectReason::Shutdown,
            ..
        }))
    ));
    assert!(rx_a.recv().await.is_none(), "the hub holds no sender after the removal");
    assert!(
        rx_b.recv().await.is_none(),
        "dropping a removed connection releases it silently"
    );
}

#[test]
async fn close_notice_on_a_full_channel_is_dropped() {
    let users = Connections::new();
    let (tx, mut rx) = mpsc::channel(1);
    let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
    users.register_connection(user_id, conn_id, key(), tx, vec![]).await;
    assert_eq!(users.send_to_connection(conn_id, ping("1")).await, Ok(true));

    users
        .remove_connection(user_id, Some(conn_id))
        .await
        .unwrap()
        .notify(disconnected(user_id, conn_id, DisconnectReason::Replaced));
    assert!(matches!(rx.recv().await, Some(HubMessage::Ping(_))));
    assert!(rx.recv().await.is_none());
}

#[test]
async fn remove_connection_cas_ignores_stale_id() {
    let users = Connections::new();
    let (tx, _rx) = mpsc::channel(1);
    let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
    users.register_connection(user_id, conn_id, key(), tx, vec![]).await;

    // stale id → no-op
    assert!(users.remove_connection(user_id, Some(Uuid::new_v4())).await.is_none());
    assert_eq!(
        users
            .remove_connection(user_id, Some(conn_id))
            .await
            .map(|removed| removed.connection_id),
        Some(conn_id)
    );
    assert_eq!(users.len().await, 0);
}
