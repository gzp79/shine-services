use crate::{
    repositories::{command_inbox::CommandInboxDb, command_log::CommandLogDb},
    services::command_stream::{
        stream_persistence::PersistenceWork, stream_relay::StreamRelay, PersistResult, StreamConfig, StreamEgress,
        StreamLogService, StreamPersistence,
    },
};
use shine_core::tokio::GraceTimer;
use shine_game::replica::{ReplayState, StreamId};
use std::sync::{Arc, Mutex};
use tokio::{
    sync::{mpsc, oneshot, Notify},
    task::JoinHandle,
    time::{self, MissedTickBehavior},
};
use tracing::{Instrument, Span};
use uuid::Uuid;

/// Only one batch per stream may be in flight, so nothing is gained by a deeper queue.
const CHANNEL_CAPACITY: usize = 1;

enum Message {
    Subscribe(Uuid),
    Unsubscribe(Uuid),
    Stop,
}

/// The two tasks of a running stream: the fast path for optimistic in-memory handling and the durable path for persistence.
pub struct StreamWorker {
    stream_id: StreamId,
    messages: mpsc::UnboundedSender<Message>,
    wake: Arc<Notify>,
    tasks: Mutex<Option<[JoinHandle<()>; 2]>>,
}

impl StreamWorker {
    /// Starts the two tasks of one stream. The caller is never blocked all heavy lifting is done asynchronously in the tasks.
    pub fn start<S, I, L, E>(
        service: StreamLogService<I, L>,
        inbox: I,
        egress: E,
        stream_id: StreamId,
        config: StreamConfig,
    ) -> Arc<StreamWorker>
    where
        S: ReplayState,
        I: CommandInboxDb + 'static,
        L: CommandLogDb + 'static,
        E: StreamEgress,
    {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let (request_tx, request_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let wake = Arc::new(Notify::new());
        let stream_config = service.config().clone();

        let persistence = StreamPersistence::<S, I, L>::new(stream_id.clone(), service);
        let durable = tokio::spawn(run_persistence(persistence, request_rx));

        let relay = StreamRelay::<S, I, E>::new(stream_id.clone(), inbox, egress, stream_config);
        let fast = tokio::spawn(run_relay(relay, config, wake.clone(), message_rx, request_tx));

        Arc::new(StreamWorker {
            stream_id,
            messages: message_tx,
            wake,
            tasks: Mutex::new(Some([fast, durable])),
        })
    }

    pub fn stream_id(&self) -> &StreamId {
        &self.stream_id
    }

    /// Adds or synchronizes a subscriber.
    pub fn subscribe(&self, connection_id: Uuid) -> bool {
        self.messages.send(Message::Subscribe(connection_id)).is_ok()
    }

    /// Removes a subscriber.
    pub fn unsubscribe(&self, connection_id: Uuid) -> bool {
        self.messages.send(Message::Unsubscribe(connection_id)).is_ok()
    }

    /// Notifies the stream that there might be new messages in the inbox.
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// Asks the stream to persist what is waiting and stop.
    pub fn stop(&self) -> bool {
        self.messages.send(Message::Stop).is_ok()
    }

    /// Returns whether the stream has finished and will not accept new messages.
    pub fn is_finished(&self) -> bool {
        self.messages.is_closed()
    }

    /// Waits until both tasks have ended and pending messages have been persisted.
    pub async fn wait_finished(&self) {
        self.messages.closed().await;
        let tasks = self.tasks.lock().unwrap().take();
        if let Some(tasks) = tasks {
            for task in tasks {
                task.await.ok();
            }
        }
    }
}

struct PersistenceRequest {
    work: PersistenceWork,
    response: oneshot::Sender<PersistResult>,
}

/// Only a batch of commands is traced as a unit of work; a reconcile, or a pass with nothing to store, is idle noise.
fn persist_span(stream_id: &StreamId, work: &PersistenceWork) -> Span {
    match work {
        PersistenceWork::Persist { commands, .. } if !commands.is_empty() => {
            tracing::info_span!("persist stream", stream_id = %stream_id, commands = commands.len())
        }
        _ => tracing::trace_span!("persist stream", stream_id = %stream_id),
    }
}

async fn run_persistence<S, I, L>(
    mut persistence: StreamPersistence<S, I, L>,
    mut requests: mpsc::Receiver<PersistenceRequest>,
) where
    S: ReplayState,
    I: CommandInboxDb + 'static,
    L: CommandLogDb + 'static,
{
    while let Some(PersistenceRequest { work, response }) = requests.recv().await {
        let span = persist_span(persistence.stream_id(), &work);
        response.send(persistence.persist(work).instrument(span).await).ok();
    }
}

/// The fast task: holds the relay back until the log has given it a history, then drives it until the
/// stream ends.
async fn run_relay<S, I, E>(
    relay: StreamRelay<S, I, E>,
    config: StreamConfig,
    wake: Arc<Notify>,
    mut receiver: mpsc::UnboundedReceiver<Message>,
    persistence: mpsc::Sender<PersistenceRequest>,
) where
    S: ReplayState,
    I: CommandInboxDb + 'static,
    E: StreamEgress,
{
    let mut relay_loop = RelayLoop {
        relay,
        persistence,
        wake,
        in_flight: None,
    };
    if relay_loop.preflight().await {
        relay_loop.run(&config, &mut receiver).await;
    }
    // Whoever was queued before the stream ended, or while it was loading, is told to start again.
    reject_queued(relay_loop.relay.egress(), relay_loop.relay.stream_id(), &mut receiver).await;
}

/// Answers the subscribers that queued up while the stream was loading or ending.
async fn reject_queued<E: StreamEgress>(
    egress: &E,
    stream_id: &StreamId,
    receiver: &mut mpsc::UnboundedReceiver<Message>,
) {
    let message = Arc::new(crate::models::messages::StreamMessage::Unsynced { stream_id: stream_id.clone() });
    receiver.close();
    while let Some(queued) = receiver.recv().await {
        if let Message::Subscribe(connection_id) = queued {
            egress.send(connection_id, message.clone()).await;
        }
    }
}

/// What drives the relay: the relay itself holds no loop, so every message, answer and wake of the
/// stream is turned into a call on it here.
///
/// Only a pass over the inbox talks to the subscribers and starts a persistence. Everything else is
/// short bookkeeping that ends with a wake to get that pass going.
struct RelayLoop<S, I, E>
where
    S: ReplayState,
    I: CommandInboxDb,
    E: StreamEgress,
{
    relay: StreamRelay<S, I, E>,
    persistence: mpsc::Sender<PersistenceRequest>,
    wake: Arc<Notify>,
    /// The answer of the batch handed to the persistence. At most one is on its way.
    in_flight: Option<oneshot::Receiver<PersistResult>>,
}

impl<S, I, E> RelayLoop<S, I, E>
where
    S: ReplayState,
    I: CommandInboxDb + 'static,
    E: StreamEgress,
{
    /// Hands the work to the persistence. `None` if the persistence is gone.
    async fn request(&self, work: PersistenceWork) -> Option<oneshot::Receiver<PersistResult>> {
        let (response, result) = oneshot::channel();
        self.persistence
            .send(PersistenceRequest { work, response })
            .await
            .ok()?;
        Some(result)
    }

    /// Reconciles the empty relay with the durable head before it may touch the inbox or a subscriber.
    /// An existing empty stream is already current; a missing or non-empty stream takes the normal
    /// reload path. Returns whether the stream is servable.
    async fn preflight(&mut self) -> bool {
        let Some(loading) = self.request(self.relay.reconcile_work()).await else {
            tracing::warn!("[{}] The persistence of the stream is gone", self.relay.stream_id());
            return false;
        };
        let Ok(result) = loading.await else {
            tracing::warn!("[{}] The persistence of the stream is gone", self.relay.stream_id());
            return false;
        };
        // Giving up at once on a failure: the relay holds nothing to serve, so there is no degraded stream
        // to keep.
        if let Err(error) = self.relay.accept(result) {
            tracing::warn!("[{}] Failed to load the stream: {error:#?}", self.relay.stream_id());
            return false;
        }
        true
    }

    async fn run(&mut self, config: &StreamConfig, receiver: &mut mpsc::UnboundedReceiver<Message>) {
        let mut poll = time::interval(config.inbox_recovery_poll_interval);
        poll.set_missed_tick_behavior(MissedTickBehavior::Skip);
        poll.tick().await;
        let wake = self.wake.clone();
        let mut close_timer = GraceTimer::new(config.worker_unload_grace);

        let mut alive = self.process_inbox().await;
        while alive {
            close_timer.arm_if(!self.relay.has_subscribers());

            tokio::select! {
                biased;

                // Bookkeeping messages from subscribers.
                message = receiver.recv() => match message {
                    Some(Message::Subscribe(connection_id)) => {
                        self.relay.subscribe(connection_id);
                        self.wake.notify_one();
                    }
                    Some(Message::Unsubscribe(connection_id)) => self.relay.unsubscribe(connection_id),
                    Some(Message::Stop) | None => break,
                },

                // Handle persistence results.
                result = async {
                    self.in_flight
                        .as_mut()
                        .expect("the result branch is disabled without an outstanding job")
                        .await
                }, if self.in_flight.is_some() => {
                    self.in_flight = None;
                    alive = match result {
                        Ok(result) => self.settle(result).await,
                        Err(_) => self.give_up("The persistence of the stream is gone").await,
                    };
                }

                // Self notifying and periodic polling.
                _ = wake.notified() => alive = self.process_inbox().await,
                _ = poll.tick() => alive = self.process_inbox().await,

                // Graceful shutdown after the no-subscriber grace period.
                _ = close_timer.wait() => break,
            }
        }

        if alive {
            // Refuse new subscriptions before making the last commands durable.
            receiver.close();
            self.drain().await;
        }
    }

    /// One look at the inbox: a page of what waits there is versioned, sent to the subscribers and, if
    /// the persistence is free, submitted. Returns whether the stream goes on.
    ///
    /// A page that may not be the last wakes the loop for the next one, which leaves room for the
    /// messages and answers in between.
    async fn process_inbox(&mut self) -> bool {
        match self.relay.ingest().await {
            Ok(true) => self.wake.notify_one(),
            Ok(false) => {}
            Err(err) => return self.give_up(format!("Processing the inbox failed: {err:#?}")).await,
        }
        self.relay.deliver().await;
        if self.in_flight.is_some() {
            return true;
        }
        let Some(work) = self.relay.work_to_persist() else {
            return true;
        };
        match self.request(work).await {
            Some(result) => {
                self.in_flight = Some(result);
                true
            }
            None => self.give_up("The persistence of the stream is gone").await,
        }
    }

    /// An answer of the persistence: only bookkeeping, the next pass tells the subscribers and submits
    /// what piled up meanwhile. Returns whether the stream goes on.
    async fn settle(&mut self, result: PersistResult) -> bool {
        if let Err(error) = self.relay.accept(result) {
            return self.give_up(format!("The persistence failed: {error:#?}")).await;
        }
        self.wake.notify_one();
        true
    }

    /// Makes durable what is waiting and waits for the answer, so that once the stream has ended, every
    /// command it took from the inbox is in the log. A command that arrives after this is left in the
    /// inbox for whoever serves the stream next.
    async fn drain(&mut self) {
        if let Err(err) = self.relay.ingest().await {
            tracing::warn!(
                "[{}] The last look at the inbox failed: {err:#?}",
                self.relay.stream_id()
            );
        }
        loop {
            if let Some(answer) = self.in_flight.take() {
                let Ok(result) = answer.await else {
                    return;
                };
                // The log holds a history of its own after a reload; the commands of this instance that are
                // not in it stay in the inbox, so they are not lost by giving up here.
                let reloaded = matches!(result, PersistResult::Reloaded { .. });
                if let Err(error) = self.relay.accept(result) {
                    tracing::warn!(
                        "[{}] The last commands could not be stored: {error:#?}",
                        self.relay.stream_id()
                    );
                    return;
                }
                if reloaded {
                    return;
                }
            }
            // Nothing is waiting and nothing is in flight: everything taken is durable.
            let Some(work) = self.relay.work_to_persist() else {
                return;
            };
            let Some(answer) = self.request(work).await else {
                return;
            };
            self.in_flight = Some(answer);
        }
    }

    /// The stream cannot go on: the subscribers are told and have to subscribe again. Returns `false`.
    async fn give_up(&mut self, reason: impl std::fmt::Display) -> bool {
        tracing::warn!("[{}] {reason}", self.relay.stream_id());
        self.relay.give_up().await;
        false
    }
}

#[cfg(test)]
#[path = "stream_worker_test.rs"]
mod stream_worker_test;
