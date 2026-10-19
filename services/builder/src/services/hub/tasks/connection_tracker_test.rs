//! Unit tests of the connection tracker and its shared consumer loop.

use super::*;
use ring::rand::SystemRandom;
use shine_test::test;
use tokio::sync::mpsc;

fn connected(user_id: Uuid, connection_id: Uuid) -> HubMessage {
    HubMessage::Hub(HubEvent::UserConnected {
        user_id,
        connection_id,
        session_key: SessionKey::new_random(&SystemRandom::new()).unwrap(),
    })
}

struct Probe {
    tx: mpsc::UnboundedSender<usize>,
}

impl ConnectionConsumer for Probe {
    async fn on_tick(&mut self, tracker: &ConnectionTracker) {
        let _ = self.tx.send(tracker.connections().len());
    }
}

#[test]
async fn loop_feeds_current_connections_to_consumer() {
    let (sub_tx, sub_rx) = mpsc::unbounded_channel();
    let (probe_tx, mut probe_rx) = mpsc::unbounded_channel();
    tokio::spawn(run_connection_loop(
        sub_rx,
        Duration::from_millis(5),
        None,
        Probe { tx: probe_tx },
    ));

    sub_tx.send(connected(Uuid::new_v4(), Uuid::new_v4())).unwrap();

    // Wait until a tick observes the tracked connection.
    let saw_one = tokio::time::timeout(Duration::from_secs(1), async {
        while probe_rx.recv().await != Some(1) {}
    })
    .await;
    assert!(
        saw_one.is_ok(),
        "consumer should observe the connection the loop event-sourced"
    );
}

/// Re-arms the wake on every tick, as a steady stream of chat appends does.
struct Rewaking {
    wake: Arc<Notify>,
    tx: mpsc::UnboundedSender<usize>,
}

impl ConnectionConsumer for Rewaking {
    async fn on_tick(&mut self, tracker: &ConnectionTracker) {
        self.wake.notify_one();
        let _ = self.tx.send(tracker.connections().len());
        tokio::task::yield_now().await;
    }
}

#[test]
async fn steady_wakes_do_not_starve_lifecycle_events() {
    let (sub_tx, sub_rx) = mpsc::unbounded_channel();
    let (probe_tx, mut probe_rx) = mpsc::unbounded_channel();
    let wake = Arc::new(Notify::new());
    // No periodic tick within the test: only wakes drive the consumer.
    tokio::spawn(run_connection_loop(
        sub_rx,
        Duration::from_secs(3600),
        Some(wake.clone()),
        Rewaking {
            wake: wake.clone(),
            tx: probe_tx,
        },
    ));
    wake.notify_one();
    assert!(
        tokio::time::timeout(Duration::from_secs(1), probe_rx.recv())
            .await
            .is_ok(),
        "the wake runs a tick"
    );

    let connected_users = 3;
    for _ in 0..connected_users {
        sub_tx.send(connected(Uuid::new_v4(), Uuid::new_v4())).unwrap();
    }
    let saw_all = tokio::time::timeout(Duration::from_secs(1), async {
        while probe_rx.recv().await != Some(connected_users) {}
    })
    .await;
    assert!(
        saw_all.is_ok(),
        "lifecycle events must be applied even while the wake is always ready"
    );
}

/// Takes longer than its interval to run, as a consumer does under load.
struct Slow {
    tx: mpsc::UnboundedSender<usize>,
}

impl ConnectionConsumer for Slow {
    async fn on_tick(&mut self, tracker: &ConnectionTracker) {
        let _ = self.tx.send(tracker.connections().len());
        time::sleep(Duration::from_millis(30)).await;
    }
}

#[test]
async fn ticks_longer_than_the_interval_do_not_starve_lifecycle_events() {
    let (sub_tx, sub_rx) = mpsc::unbounded_channel();
    let (probe_tx, mut probe_rx) = mpsc::unbounded_channel();
    tokio::spawn(run_connection_loop(
        sub_rx,
        Duration::from_millis(10),
        None,
        Slow { tx: probe_tx },
    ));
    assert!(tokio::time::timeout(Duration::from_secs(1), probe_rx.recv()).await.is_ok());

    for _ in 0..3 {
        sub_tx.send(connected(Uuid::new_v4(), Uuid::new_v4())).unwrap();
    }
    let saw_all = tokio::time::timeout(Duration::from_secs(2), async {
        while probe_rx.recv().await != Some(3) {}
    })
    .await;
    assert!(
        saw_all.is_ok(),
        "the view must be updated even when the ticker is always due"
    );
}

#[test]
async fn a_closed_subscription_ends_the_loop() {
    let (sub_tx, sub_rx) = mpsc::unbounded_channel();
    let (probe_tx, _probe_rx) = mpsc::unbounded_channel();
    let task = tokio::spawn(run_connection_loop(
        sub_rx,
        Duration::from_secs(3600),
        None,
        Probe { tx: probe_tx },
    ));
    drop(sub_tx);
    assert!(tokio::time::timeout(Duration::from_secs(1), task).await.is_ok());
}

#[test]
async fn a_shutdown_event_ends_the_loop_even_with_events_queued_behind_it() {
    let (sub_tx, sub_rx) = mpsc::unbounded_channel();
    let (probe_tx, _probe_rx) = mpsc::unbounded_channel();
    sub_tx.send(HubMessage::Hub(HubEvent::Shutdown)).unwrap();
    sub_tx.send(connected(Uuid::new_v4(), Uuid::new_v4())).unwrap();
    let task = tokio::spawn(run_connection_loop(
        sub_rx,
        Duration::from_secs(3600),
        None,
        Probe { tx: probe_tx },
    ));
    assert!(tokio::time::timeout(Duration::from_secs(1), task).await.is_ok());
}

/// Re-arms the wake on every tick and counts them.
struct Counting {
    wake: Arc<Notify>,
    ticks: Arc<std::sync::atomic::AtomicUsize>,
}

impl ConnectionConsumer for Counting {
    async fn on_tick(&mut self, _tracker: &ConnectionTracker) {
        self.ticks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.wake.notify_one();
    }
}

#[test]
async fn steady_wakes_are_spaced_out() {
    let (_sub_tx, sub_rx) = mpsc::unbounded_channel();
    let wake = Arc::new(Notify::new());
    let ticks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    tokio::spawn(run_connection_loop(
        sub_rx,
        Duration::from_secs(3600),
        Some(wake.clone()),
        Counting { wake: wake.clone(), ticks: ticks.clone() },
    ));
    wake.notify_one();
    time::sleep(Duration::from_millis(500)).await;

    let ticks = ticks.load(std::sync::atomic::Ordering::Relaxed);
    let max = 500 / WAKE_MIN_SPACING.as_millis() as usize + 2;
    assert!(ticks >= 3, "wakes must keep driving the consumer, got {ticks} ticks");
    assert!(ticks <= max, "wake ticks must be spaced by {WAKE_MIN_SPACING:?}, got {ticks} ticks");
}
