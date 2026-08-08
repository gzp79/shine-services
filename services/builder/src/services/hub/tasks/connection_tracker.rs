use crate::{
    models::messages::{HubEvent, HubMessage, TopicKey},
    services::HubService,
};
use shine_infra::session::SessionKey;
use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, Notify},
    task::JoinHandle,
    time,
};
use uuid::Uuid;

pub type TrackedConnection = (Uuid, SessionKey);

/// Owned, event-sourced view of the connections a single consumer's loop cares about.
pub struct ConnectionTracker {
    connections: HashMap<Uuid, TrackedConnection>,
}

impl ConnectionTracker {
    fn new() -> Self {
        Self { connections: HashMap::new() }
    }

    /// Applies one lifecycle event to the owned map. Returns `true` on a shutdown event.
    fn apply(&mut self, message: HubMessage) -> bool {
        match message {
            HubMessage::Hub(HubEvent::UserConnected {
                user_id,
                connection_id,
                session_key,
            }) => {
                self.connections.insert(user_id, (connection_id, session_key));
                false
            }
            HubMessage::Hub(HubEvent::UserDisconnected { user_id, connection_id }) => {
                // Only forget the user when the disconnected connection is the one we track; a
                // stale disconnect for a replaced connection must not drop the fresh reconnect.
                if self.connections.get(&user_id).map(|(id, _)| *id) == Some(connection_id) {
                    self.connections.remove(&user_id);
                }
                false
            }
            HubMessage::Hub(HubEvent::Shutdown) => true,
            _ => false,
        }
    }

    /// The current connections, keyed `user_id -> (connection_id, session_key)`.
    pub fn connections(&self) -> &HashMap<Uuid, TrackedConnection> {
        &self.connections
    }
}

/// A periodic consumer of the connection view, providing only the per-tick work.
pub trait ConnectionConsumer: Send + 'static {
    fn on_tick(&mut self, tracker: &ConnectionTracker) -> impl Future<Output = ()> + Send;
}

/// Subscribes a consumer to the hub lifecycle bus and spawns its [`run_connection_loop`], returning
/// the loop's handle.
pub async fn spawn_connection_loop<C: ConnectionConsumer>(
    service: &HubService,
    interval: Duration,
    consumer: C,
) -> JoinHandle<()> {
    // Subscribe synchronously before spawning: an event dispatched before a consumer subscribes is
    // lost to it forever, so every consumer must be subscribed before the command loop dispatches.
    let subscription = service.subscribe(vec![TopicKey::Hub]).await;
    tokio::spawn(run_connection_loop(subscription, interval, None, consumer))
}

/// Like [`spawn_connection_loop`], but the loop also runs an extra `on_tick` whenever `wake` is
/// notified.
pub async fn spawn_connection_loop_with_wake<C: ConnectionConsumer>(
    service: &HubService,
    interval: Duration,
    wake: Arc<Notify>,
    consumer: C,
) -> JoinHandle<()> {
    let subscription = service.subscribe(vec![TopicKey::Hub]).await;
    tokio::spawn(run_connection_loop(subscription, interval, Some(wake), consumer))
}

/// Drives a self-contained loop that event-sources its own [`ConnectionTracker`] from
/// `subscription` and invokes `consumer.on_tick` every `interval` and on each `wake`. A closed
/// subscription or a `Shutdown` event ends the loop.
pub async fn run_connection_loop<C: ConnectionConsumer>(
    mut subscription: mpsc::UnboundedReceiver<HubMessage>,
    interval: Duration,
    wake: Option<Arc<Notify>>,
    mut consumer: C,
) {
    let mut tracker = ConnectionTracker::new();
    let mut ticker = time::interval(interval);
    // Delay (not Burst) so a slow tick reschedules a full interval ahead instead of firing missed
    // ticks back-to-back, keeping the cadence honest against the TTL margin.
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    ticker.tick().await; // skip the immediate first tick
    loop {
        // A future that resolves on a wake notification, or never if this loop has no wake source.
        // `Notify` stores a single permit, so notifications arriving during an `on_tick` collapse
        // into one follow-up pass — the coalescing that keeps a burst of appends to one delivery.
        let woken = async {
            match &wake {
                Some(wake) => wake.notified().await,
                None => std::future::pending().await,
            }
        };

        tokio::select! {
            // The tick is prioritized so a heavy event stream cannot starve TTL refreshes and let a
            // live connection expire.
            biased;
            _ = ticker.tick() => {
                consumer.on_tick(&tracker).await;
            }
            _ = woken => {
                consumer.on_tick(&tracker).await;
            }
            message = subscription.recv() => match message {
                Some(message) => {
                    if tracker.apply(message) {
                        break;
                    }
                }
                // Channel closed without a Shutdown event: stop instead of busy-looping.
                None => break,
            },
        }
    }
}

#[cfg(test)]
mod tests {
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
}
