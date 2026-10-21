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

/// Always takes longer than the interval, so under `MissedTickBehavior::Delay` the ticker is
/// already due again the instant it returns.
struct SlowTick {
    sleep: Duration,
    tx: mpsc::UnboundedSender<usize>,
}

impl ConnectionConsumer for SlowTick {
    async fn on_tick(&mut self, tracker: &ConnectionTracker) {
        let _ = self.tx.send(tracker.connections().len());
        tokio::time::sleep(self.sleep).await;
    }
}

#[test]
async fn ticks_longer_than_the_interval_still_apply_lifecycle_events() {
    let (sub_tx, sub_rx) = mpsc::unbounded_channel();
    let (probe_tx, mut probe_rx) = mpsc::unbounded_channel();
    let interval = Duration::from_millis(20);
    tokio::spawn(run_connection_loop(
        sub_rx,
        interval,
        None,
        SlowTick {
            sleep: interval * 3,
            tx: probe_tx,
        },
    ));

    // Wait for the first (empty) tick to start, so the message below is sent while that tick is
    // still sleeping, not before the loop's first deadline — otherwise `subscription.recv()` would
    // just win the very first select by having been ready before the ticker was ever due.
    assert_eq!(probe_rx.recv().await, Some(0));
    sub_tx.send(connected(Uuid::new_v4(), Uuid::new_v4())).unwrap();

    // From here on, every tick sleeps three times its own interval, so `ticker.tick()` is already
    // due again the instant it returns. A loop biased towards the ticker alone would keep firing
    // it forever without ever polling the subscription again; the fix must drain pending lifecycle
    // events before each due tick regardless.
    let saw_one = tokio::time::timeout(Duration::from_secs(2), async {
        while probe_rx.recv().await != Some(1) {}
    })
    .await;
    assert!(
        saw_one.is_ok(),
        "a tick that always runs over its interval must not starve lifecycle events"
    );
}
