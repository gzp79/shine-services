use crate::{
    models::messages::{HubEvent, HubMessage, TopicKey},
    services::hub::{hub_metrics::hub_metrics, hub_subscribers::Subscribers},
};
use shine_core::tokio::TaskSupervisor;
use shine_infra::session::SessionKey;
use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, Notify},
    task::JoinHandle,
    time,
};
use uuid::Uuid;

/// A task that runs beside the hub's command loop, and that the loop stops when it ends.
pub struct BackgroundJoinHandler {
    name: &'static str,
    handle: JoinHandle<()>,
    abort: bool,
}

impl BackgroundJoinHandler {
    /// A task that is aborted when the loop ends.
    pub fn abortable(name: &'static str, handle: JoinHandle<()>) -> Self {
        Self { name, handle, abort: true }
    }

    /// A task that has to finish its work when the loop ends: it is awaited, never aborted.
    pub fn run_to_end(name: &'static str, handle: JoinHandle<()>) -> Self {
        Self { name, handle, abort: false }
    }

    /// Asks the task to stop now, unless it has to run to its end.
    pub fn request_stop(&self) {
        if self.abort {
            self.handle.abort();
        }
    }

    /// Waits until the task has ended. A task that was aborted is not an error.
    pub async fn join(self) {
        if let Err(err) = self.handle.await {
            if !err.is_cancelled() {
                tracing::error!("Hub task '{}' failed while stopping: {err}", self.name);
            }
        }
    }
}

/// A periodic consumer of the connection view, providing only the per-tick work.
pub trait BackgroundTask: Send + 'static {
    /// The work of a tick.
    fn on_tick(&mut self, tracker: &ConnectionTracker) -> impl Future<Output = ()> + Send;

    fn name() -> &'static str
    where
        Self: Sized,
    {
        std::any::type_name::<Self>().rsplit("::").next().unwrap_or_default()
    }
}

/// Owned, event-sourced view of the connections a single consumer's loop cares about.
pub struct ConnectionTracker {
    connections: HashMap<Uuid, (Uuid, SessionKey)>,
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
            HubMessage::Hub(HubEvent::UserDisconnected { user_id, connection_id, reason }) => {
                // Only forget the user when the disconnected connection is the one we track; a
                // stale disconnect for a replaced connection must not drop the fresh reconnect.
                if self.connections.get(&user_id).map(|(id, _)| *id) == Some(connection_id) {
                    tracing::debug!("[{user_id}] [{connection_id}] Connection forgotten: {reason}");
                    self.connections.remove(&user_id);
                }
                false
            }
            HubMessage::Hub(HubEvent::Shutdown) => true,
            _ => false,
        }
    }

    /// The current connections, keyed `user_id -> (connection_id, session_key)`.
    pub fn connections(&self) -> &HashMap<Uuid, (Uuid, SessionKey)> {
        &self.connections
    }
}

/// Short type name of a consumer, for metric labels and logs.
fn consumer_name<C: BackgroundTask>() -> &'static str {
    std::any::type_name::<C>().rsplit("::").next().unwrap_or_default()
}

/// Runs a single tick and measures its duration, labelled by the consumer's type name.
async fn run_tick<C: BackgroundTask>(consumer: &mut C, tracker: &ConnectionTracker) {
    let start = std::time::Instant::now();
    consumer.on_tick(tracker).await;
    let name = consumer_name::<C>();
    hub_metrics().tick_finished(name, start.elapsed());
}

/// Starts the background task and calls `on_tick` every `interval` and on each `wake`.
pub async fn run_background_task<C: BackgroundTask>(
    subscribers: &Subscribers,
    supervisor: &TaskSupervisor,
    interval: Duration,
    wake: Option<Arc<Notify>>,
    consumer: C,
) -> BackgroundJoinHandler {
    let subscription = subscribers.subscribe(vec![TopicKey::Hub]).await;
    let handle = supervisor.spawn(
        format!("hub {}", C::name()),
        background_task_loop(subscription, interval, wake, consumer),
    );
    BackgroundJoinHandler::abortable(C::name(), handle)
}

/// The loop of a background task. A closed subscription or a `Shutdown` event ends it.
async fn background_task_loop<C: BackgroundTask>(
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
                run_tick(&mut consumer, &tracker).await;
            }
        }
    }
}

#[cfg(test)]
#[path = "background_task_test.rs"]
mod background_task_test;
