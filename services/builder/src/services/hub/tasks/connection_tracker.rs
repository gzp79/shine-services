use crate::{
    models::messages::{HubEvent, HubMessage, TopicKey},
    services::{hub::hub_metrics::hub_metrics, HubService},
};
use shine_infra::{session::SessionKey, telemetry::label};
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
            HubMessage::Hub(HubEvent::UserDisconnected { user_id, connection_id, .. }) => {
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
    /// The work of a tick.
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

/// Short type name of a consumer, for metric labels and logs.
fn consumer_name<C: ConnectionConsumer>() -> &'static str {
    std::any::type_name::<C>().rsplit("::").next().unwrap_or_default()
}

/// Max lifecycle events drained per due tick, so a flood of connects/disconnects cannot stall a tick
/// indefinitely either.
const MAX_DRAIN_PER_TICK: usize = 1_024;

/// Applies every already-queued lifecycle event, without waiting for more. A tick whose own work
/// takes at least one interval would otherwise be due again the instant it returns — and under a
/// biased select that keeps picking it over `subscription.recv()`, the connection view would never
/// advance for as long as that keeps happening. Returns `true` on a shutdown event.
fn drain_pending(subscription: &mut mpsc::UnboundedReceiver<HubMessage>, tracker: &mut ConnectionTracker) -> bool {
    for _ in 0..MAX_DRAIN_PER_TICK {
        match subscription.try_recv() {
            Ok(message) => {
                if tracker.apply(message) {
                    return true;
                }
            }
            Err(_) => break,
        }
    }
    false
}

/// Runs a single tick and measures its duration, labelled by the consumer's type name.
async fn run_tick<C: ConnectionConsumer>(consumer: &mut C, tracker: &ConnectionTracker) {
    let start = std::time::Instant::now();
    consumer.on_tick(tracker).await;
    let name = consumer_name::<C>();
    hub_metrics()
        .tick_duration
        .record(start.elapsed().as_secs_f64(), &[label("consumer", name)]);
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
            // live connection expire. Lifecycle events come before wakes: a wake permit stored during
            // `on_tick` is ready at once, so steady wakes would otherwise starve the connection view.
            biased;
            _ = ticker.tick() => {
                if drain_pending(&mut subscription, &mut tracker) {
                    tracing::info!("{} loop stopped on hub shutdown", consumer_name::<C>());
                    break;
                }
                run_tick(&mut consumer, &tracker).await;
            }
            message = subscription.recv() => match message {
                Some(message) => {
                    if tracker.apply(message) {
                        tracing::info!("{} loop stopped on hub shutdown", consumer_name::<C>());
                        break;
                    }
                }
                // Channel closed without a Shutdown event: stop instead of busy-looping.
                None => {
                    tracing::warn!("{} loop stopped: hub subscription closed", consumer_name::<C>());
                    break;
                }
            },
            _ = woken => {
                if drain_pending(&mut subscription, &mut tracker) {
                    tracing::info!("{} loop stopped on hub shutdown", consumer_name::<C>());
                    break;
                }
                run_tick(&mut consumer, &tracker).await;
            }
        }
    }
}

#[cfg(test)]
#[path = "connection_tracker_test.rs"]
mod connection_tracker_test;
