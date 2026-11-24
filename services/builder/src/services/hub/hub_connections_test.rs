//! Unit tests of [`Connections`]: send outcomes, topic filter, CAS removal, close reason.

use crate::{
    models::messages::{DisconnectReason, HubMessage, TopicKey},
    services::{
        hub::hub_connections::{Connections, DeadConnection},
        socket_channel,
    },
    testing::{hub::TestSession, TestMessage},
};
use shine_test::test;
use uuid::Uuid;

#[test]
async fn directed_send_moves_to_the_one_connection() {
    let users = Connections::new();
    let (tx, mut rx) = socket_channel(4);
    let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
    users
        .register_connection(
            user_id,
            conn_id,
            TestSession::new_random_key(),
            tx,
            vec![TopicKey::Ping],
        )
        .await;

    assert!(users
        .send_to_connection(conn_id, TestMessage::new_ping("hi"))
        .await
        .is_ok());
    assert!(users.send_to_user(user_id, TestMessage::new_ping("yo")).await.is_ok());

    assert!(matches!(rx.recv().await, Some(HubMessage::Ping(m)) if m.id == "hi"));
    assert!(matches!(rx.recv().await, Some(HubMessage::Ping(m)) if m.id == "yo"));
}

#[test]
async fn directed_send_to_unknown_target_is_ok_noop() {
    let users = Connections::new();
    assert_eq!(
        users
            .send_to_connection(Uuid::new_v4(), TestMessage::new_ping("x"))
            .await,
        Ok(false)
    );
    assert!(users
        .send_to_user(Uuid::new_v4(), TestMessage::new_ping("x"))
        .await
        .is_ok());
}

#[test]
async fn send_to_user_follows_the_active_connection_and_reports_dead_ones() {
    let users = Connections::new();
    let user_id = Uuid::new_v4();
    let (old_tx, mut old_rx) = socket_channel(4);
    let (new_tx, mut new_rx) = socket_channel(1);
    let (old_id, new_id) = (Uuid::new_v4(), Uuid::new_v4());
    users
        .register_connection(user_id, old_id, TestSession::new_random_key(), old_tx, vec![])
        .await;
    users
        .register_connection(user_id, new_id, TestSession::new_random_key(), new_tx, vec![])
        .await;

    // The user's latest registration is its active connection.
    assert!(users.send_to_user(user_id, TestMessage::new_ping("1")).await.is_ok());
    assert!(matches!(new_rx.try_recv(), Ok(HubMessage::Ping(m)) if m.id == "1"));
    assert!(old_rx.try_recv().is_err());

    assert!(users.send_to_user(user_id, TestMessage::new_ping("2")).await.is_ok());
    assert_eq!(
        users.send_to_user(user_id, TestMessage::new_ping("3")).await,
        Err(DeadConnection {
            user_id,
            connection_id: new_id,
            reason: DisconnectReason::SlowConsumer
        })
    );
    // The connection ended with the first failure: it is not reported again.
    drop(new_rx);
    assert_eq!(users.send_to_user(user_id, TestMessage::new_ping("4")).await, Ok(()));
}

#[test]
async fn full_channel_reports_dead_connection() {
    let users = Connections::new();
    let (tx, _rx) = socket_channel(1);
    let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
    users
        .register_connection(
            user_id,
            conn_id,
            TestSession::new_random_key(),
            tx,
            vec![TopicKey::Ping],
        )
        .await;

    assert!(users
        .send_to_connection(conn_id, TestMessage::new_ping("1"))
        .await
        .is_ok()); // fills capacity 1
    assert_eq!(
        users.send_to_connection(conn_id, TestMessage::new_ping("2")).await,
        Err(DeadConnection {
            user_id,
            connection_id: conn_id,
            reason: DisconnectReason::SlowConsumer
        })
    );
}

#[test]
async fn closed_channel_reports_dead_connection() {
    let users = Connections::new();
    let (tx, rx) = socket_channel(1);
    let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
    users
        .register_connection(
            user_id,
            conn_id,
            TestSession::new_random_key(),
            tx,
            vec![TopicKey::Ping],
        )
        .await;
    drop(rx);
    assert_eq!(
        users.send_to_connection(conn_id, TestMessage::new_ping("1")).await,
        Err(DeadConnection {
            user_id,
            connection_id: conn_id,
            reason: DisconnectReason::DeadChannel
        })
    );
}

#[test]
async fn broadcast_clones_to_matching_topic_only() {
    let users = Connections::new();
    let (tx_chat, mut rx_chat) = socket_channel(4);
    let (tx_hub, mut rx_hub) = socket_channel(4);
    let chat_conn = Uuid::new_v4();
    let hub_conn = Uuid::new_v4();
    users
        .register_connection(
            Uuid::new_v4(),
            chat_conn,
            TestSession::new_random_key(),
            tx_chat,
            vec![TopicKey::Ping],
        )
        .await;
    users
        .register_connection(
            Uuid::new_v4(),
            hub_conn,
            TestSession::new_random_key(),
            tx_hub,
            vec![TopicKey::Hub],
        )
        .await;

    assert!(users.broadcast(TestMessage::new_ping("broadcast")).await.is_ok());
    assert!(matches!(rx_chat.recv().await, Some(HubMessage::Ping(_))));
    // Hub-topic connection must not receive a Chat broadcast.
    assert!(rx_hub.try_recv().is_err());
}

#[test]
async fn removed_connection_releases_its_endpoint_after_the_reason() {
    let users = Connections::new();
    let (tx_a, mut rx_a) = socket_channel(4);
    let (tx_b, mut rx_b) = socket_channel(4);
    let (user_a, conn_a) = (Uuid::new_v4(), Uuid::new_v4());
    users
        .register_connection(user_a, conn_a, TestSession::new_random_key(), tx_a, vec![])
        .await;
    users
        .register_connection(
            Uuid::new_v4(),
            Uuid::new_v4(),
            TestSession::new_random_key(),
            tx_b,
            vec![],
        )
        .await;

    let mut removed = users.unregister_all().await;
    assert_eq!(removed.len(), 2);
    assert_eq!(users.len().await, 0);
    removed.sort_by_key(|r| r.connection_id != conn_a);
    let mut removed = removed.into_iter();
    let first = removed.next().unwrap();
    assert_eq!((first.user_id, first.connection_id), (user_a, conn_a));
    first.close(DisconnectReason::Shutdown);
    drop(removed);

    assert_eq!(rx_a.close_reason(), Some(DisconnectReason::Shutdown));
    assert!(rx_a.recv().await.is_none(), "the hub holds no sender after the removal");
    assert!(rx_b.recv().await.is_none(), "dropping a removed connection releases it");
    assert_eq!(rx_b.close_reason(), None);
}

#[test]
async fn the_close_reason_reaches_a_full_connection() {
    let users = Connections::new();
    let (tx, mut rx) = socket_channel(1);
    let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
    users
        .register_connection(user_id, conn_id, TestSession::new_random_key(), tx, vec![])
        .await;
    assert_eq!(
        users.send_to_connection(conn_id, TestMessage::new_ping("1")).await,
        Ok(true)
    );

    users
        .unregister_connection(user_id, Some(conn_id))
        .await
        .unwrap()
        .close(DisconnectReason::Replaced);

    assert_eq!(rx.close_reason(), Some(DisconnectReason::Replaced));
    assert!(matches!(rx.recv().await, Some(HubMessage::Ping(_))));
    assert!(rx.recv().await.is_none());
}

#[test]
async fn a_dead_connection_is_reported_to_the_first_send_only() {
    let users = Connections::new();
    let (tx, _rx) = socket_channel(1);
    let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
    users
        .register_connection(
            user_id,
            conn_id,
            TestSession::new_random_key(),
            tx,
            vec![TopicKey::Ping],
        )
        .await;
    assert_eq!(
        users.send_to_connection(conn_id, TestMessage::new_ping("1")).await,
        Ok(true)
    );

    assert_eq!(
        users.send_to_connection(conn_id, TestMessage::new_ping("2")).await,
        Err(DeadConnection {
            user_id,
            connection_id: conn_id,
            reason: DisconnectReason::SlowConsumer
        })
    );
    // Every later send finds the connection ended: nothing is queued and nothing is reported.
    assert_eq!(
        users.send_to_connection(conn_id, TestMessage::new_ping("3")).await,
        Ok(false)
    );
    assert_eq!(users.send_to_user(user_id, TestMessage::new_ping("4")).await, Ok(()));
    assert_eq!(users.broadcast(TestMessage::new_ping("5")).await, Ok(()));
}

#[test]
async fn a_connection_ended_by_someone_else_takes_no_more_messages() {
    let users = Connections::new();
    let (tx, mut rx) = socket_channel(4);
    let closer = tx.clone();
    let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
    users
        .register_connection(
            user_id,
            conn_id,
            TestSession::new_random_key(),
            tx,
            vec![TopicKey::Ping],
        )
        .await;
    assert!(closer.close(DisconnectReason::RateLimited));

    assert_eq!(
        users.send_to_connection(conn_id, TestMessage::new_ping("1")).await,
        Ok(false)
    );
    assert!(rx.try_recv().is_err());
}

#[test]
async fn remove_connection_cas_ignores_stale_id() {
    let users = Connections::new();
    let (tx, _rx) = socket_channel(1);
    let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
    users
        .register_connection(user_id, conn_id, TestSession::new_random_key(), tx, vec![])
        .await;

    // stale id → no-op
    assert!(users
        .unregister_connection(user_id, Some(Uuid::new_v4()))
        .await
        .is_none());
    assert_eq!(
        users
            .unregister_connection(user_id, Some(conn_id))
            .await
            .map(|removed| removed.connection_id),
        Some(conn_id)
    );
    assert_eq!(users.len().await, 0);
}
