use super::*;
use crate::{models::messages::HubEvent, testing::TestMessage};
use shine_test::test;
use std::time::Duration;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(5);

impl SocketReceiver {
    /// The next queued message, whatever the reason; `None` when the queue is empty or closed.
    pub fn try_recv(&mut self) -> Result<HubMessage, mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }

    /// The next queued message; `None` when the queue is closed.
    pub async fn recv(&mut self) -> Option<HubMessage> {
        self.rx.recv().await
    }

    /// The reason the connection ended with, if it did.
    pub fn close_reason(&self) -> Option<DisconnectReason> {
        *self.close.borrow()
    }
}

fn ping_id(event: SocketMessage) -> String {
    match event {
        SocketMessage::Data(HubMessage::Ping(ping)) => ping.id,
        other => panic!("expected a ping, got {other:?}"),
    }
}

#[test]
async fn messages_come_in_order() {
    let (tx, mut egress) = socket_channel(4);
    tx.try_send(TestMessage::new_ping("1")).unwrap();
    tx.try_send(TestMessage::new_ping("2")).unwrap();

    assert_eq!(ping_id(egress.next().await), "1");
    assert_eq!(ping_id(egress.next().await), "2");
}

#[test]
async fn the_close_reason_beats_queued_messages() {
    let (tx, mut egress) = socket_channel(4);
    tx.try_send(TestMessage::new_ping("1")).unwrap();
    tx.try_send(TestMessage::new_ping("2")).unwrap();

    assert!(tx.close(DisconnectReason::Replaced));

    assert!(matches!(
        egress.next().await,
        SocketMessage::Close(DisconnectReason::Replaced)
    ));
}

#[test]
async fn a_full_queue_still_delivers_the_reason() {
    let (tx, mut egress) = socket_channel(1);
    tx.try_send(TestMessage::new_ping("1")).unwrap();
    assert!(matches!(
        tx.try_send(TestMessage::new_ping("2")),
        Err(TrySendError::Full(_))
    ));

    assert!(tx.close(DisconnectReason::SlowConsumer));

    assert!(matches!(
        egress.next().await,
        SocketMessage::Close(DisconnectReason::SlowConsumer)
    ));
}

#[test]
async fn only_the_first_close_wins_across_clones() {
    let (tx, egress) = socket_channel(1);
    let other = tx.clone();

    assert!(tx.close(DisconnectReason::RateLimited));
    assert!(!other.close(DisconnectReason::Replaced));
    assert!(!tx.close(DisconnectReason::Shutdown));

    assert_eq!(other.close_reason(), Some(DisconnectReason::RateLimited));
    assert_eq!(egress.close_reason(), Some(DisconnectReason::RateLimited));
}

#[test]
async fn a_closed_queue_still_tells_the_first_closer() {
    let (tx, egress) = socket_channel(1);
    drop(egress);

    assert!(matches!(
        tx.try_send(TestMessage::new_ping("1")),
        Err(TrySendError::Closed(_))
    ));
    // The slot works without a reader: it is what makes one dead connection be reported once.
    assert!(tx.close(DisconnectReason::DeadChannel));
    assert!(!tx.close(DisconnectReason::DeadChannel));
    assert_eq!(tx.close_reason(), Some(DisconnectReason::DeadChannel));
}

#[test]
async fn next_waits_for_the_close() {
    let (tx, mut egress) = socket_channel(1);
    let waiting = tokio::spawn(async move { egress.next().await });
    tokio::task::yield_now().await;

    tx.close(DisconnectReason::SessionInvalid);

    let event = timeout(WAIT, waiting).await.unwrap().unwrap();
    assert!(matches!(event, SocketMessage::Close(DisconnectReason::SessionInvalid)));
}

#[test]
async fn a_dropped_sender_without_a_reason_is_gone() {
    let (tx, mut egress) = socket_channel(2);
    tx.try_send(TestMessage::new_ping("1")).unwrap();
    drop(tx);

    // What was queued is still delivered, then the endpoint reports the missing reason.
    assert_eq!(ping_id(egress.next().await), "1");
    assert!(matches!(egress.next().await, SocketMessage::Gone));
}

#[test]
async fn a_reason_set_before_the_sender_is_dropped_is_delivered() {
    let (tx, mut egress) = socket_channel(2);
    tx.close(DisconnectReason::Client);
    drop(tx);

    assert!(matches!(
        egress.next().await,
        SocketMessage::Close(DisconnectReason::Client)
    ));
}

#[test]
async fn a_weak_sender_does_not_keep_the_endpoint_alive() {
    let (tx, mut egress) = socket_channel(2);
    let weak = tx.downgrade();
    assert!(weak.upgrade().is_some());

    tx.close(DisconnectReason::Shutdown);
    drop(tx);

    assert!(weak.upgrade().is_none());
    assert!(matches!(
        egress.next().await,
        SocketMessage::Close(DisconnectReason::Shutdown)
    ));
}

#[test]
async fn hub_events_are_ordinary_data() {
    let (tx, mut egress) = socket_channel(2);
    tx.try_send(HubMessage::Hub(HubEvent::Shutdown)).unwrap();

    assert!(matches!(
        egress.next().await,
        SocketMessage::Data(HubMessage::Hub(HubEvent::Shutdown))
    ));
}
