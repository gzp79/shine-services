use crate::{
    repositories::{
        command_inbox::CommandInboxDb,
        command_log::{CommandLogDb, StoredCommand},
    },
    services::command_stream::{
        PersistResult, StreamActor, StreamConfig, StreamEgress, StreamLogService, StreamPersistence,
    },
};
use shine_game::replica::{ReplayState, StreamId};
use std::sync::{Arc, Mutex};
use tokio::{
    sync::{mpsc, oneshot, Notify},
    task::JoinHandle,
    time::{self, Instant, MissedTickBehavior},
};
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

        let actor = StreamActor::<S, I, E>::new(stream_id.clone(), inbox, egress, stream_config);
        let fast = tokio::spawn(run_actor(actor, config, wake.clone(), message_rx, request_tx));

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

enum PersistenceWork {
    Persist {
        baseline_version: i64,
        baseline_chain_hash: u64,
        commands: Arc<[StoredCommand]>,
    },
    Reconcile {
        version: i64,
        chain_hash: u64,
    },
}

struct PersistenceRequest {
    work: PersistenceWork,
    response: oneshot::Sender<PersistResult>,
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
        let result = match work {
            PersistenceWork::Persist {
                baseline_version,
                baseline_chain_hash,
                commands,
            } => {
                persistence
                    .persist(baseline_version, baseline_chain_hash, commands)
                    .await
            }
            PersistenceWork::Reconcile { version, chain_hash } => persistence.reconcile(version, chain_hash).await,
        };
        response.send(result).ok();
    }
}

/// The fast task: holds the actor back until the log has given it a history, then drives it until the
/// stream ends.
async fn run_actor<S, I, E>(
    actor: StreamActor<S, I, E>,
    config: StreamConfig,
    wake: Arc<Notify>,
    mut receiver: mpsc::UnboundedReceiver<Message>,
    persistence: mpsc::Sender<PersistenceRequest>,
) where
    S: ReplayState,
    I: CommandInboxDb + 'static,
    E: StreamEgress,
{
    let stream_id = actor.stream_id().clone();
    let reconcile_deadline = Instant::now() + config.backstop;
    let mut loop_state = ActorLoop {
        actor,
        persistence,
        wake,
        failures: 0,
        saturated: false,
        grace_deadline: Some(Instant::now() + config.unload_grace),
        reconcile_deadline,
        config,
    };

    if !loop_state.preflight().await {
        reject_queued(loop_state.actor.egress(), &stream_id, &mut receiver).await;
        return;
    }
    loop_state.reconcile_deadline = Instant::now() + loop_state.config.backstop;

    let mut persistence_result = None;
    loop_state.run(&mut persistence_result, &mut receiver).await;
}

/// Answers the subscribers that queued up while the stream was loading or being given up.
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

/// What drives the actor: the actor itself holds no loop, so every wake, answer and message of the
/// stream is turned into a call on it here.
struct ActorLoop<S, I, E>
where
    S: ReplayState,
    I: CommandInboxDb,
    E: StreamEgress,
{
    actor: StreamActor<S, I, E>,
    persistence: mpsc::Sender<PersistenceRequest>,
    config: StreamConfig,
    wake: Arc<Notify>,
    failures: u32,
    /// Whether the last pass found the optimistic backlog full, so the change is logged once.
    saturated: bool,
    /// When the stream ends if it has no subscriber by then.
    grace_deadline: Option<Instant>,
    /// A quiet stream still checks that another worker did not advance and trim the durable history
    /// before its wake was observed.
    reconcile_deadline: Instant,
}

impl<S, I, E> ActorLoop<S, I, E>
where
    S: ReplayState,
    I: CommandInboxDb + 'static,
    E: StreamEgress,
{
    async fn request_persist(
        &self,
        baseline_version: i64,
        baseline_chain_hash: u64,
        commands: Arc<[StoredCommand]>,
    ) -> Result<oneshot::Receiver<PersistResult>, Arc<[StoredCommand]>> {
        let (response, result) = oneshot::channel();
        self.persistence
            .send(PersistenceRequest {
                work: PersistenceWork::Persist {
                    baseline_version,
                    baseline_chain_hash,
                    commands,
                },
                response,
            })
            .await
            .map_err(|error| match error.0.work {
                PersistenceWork::Persist { commands, .. } => commands,
                PersistenceWork::Reconcile { .. } => unreachable!(),
            })?;
        Ok(result)
    }

    async fn request_reconciliation(
        &self,
        version: i64,
        chain_hash: u64,
    ) -> Result<oneshot::Receiver<PersistResult>, ()> {
        let (response, result) = oneshot::channel();
        self.persistence
            .send(PersistenceRequest {
                work: PersistenceWork::Reconcile { version, chain_hash },
                response,
            })
            .await
            .map_err(|_| ())?;
        Ok(result)
    }

    /// Reconciles the empty actor with the durable head before it may touch the inbox or a subscriber.
    /// An existing empty stream is already current; a missing or non-empty stream takes the normal
    /// reload path. Returns whether the stream is servable.
    async fn preflight(&mut self) -> bool {
        let (version, chain_hash) = self.actor.confirmed_head();
        let loading = match self.request_reconciliation(version, chain_hash).await {
            Ok(loading) => loading,
            Err(()) => {
                tracing::warn!("[{}] The persistence of the stream is gone", self.actor.stream_id());
                return false;
            }
        };
        match loading.await {
            Ok(PersistResult::Reloaded {
                snapshot, tail, stored_inbox, ..
            }) => {
                self.actor.accept_reload(snapshot, tail.to_vec(), stored_inbox);
                true
            }
            Ok(PersistResult::Current) => true,
            // Giving up at once: the actor holds nothing to serve, so there is no degraded stream to
            // keep. `failed` is for a stream that has a history and may yet recover.
            Ok(PersistResult::Failed { error }) => {
                tracing::warn!("[{}] Failed to load the stream: {error:#?}", self.actor.stream_id());
                false
            }
            Ok(PersistResult::Advanced { .. }) => {
                unreachable!("preflight submits reconciliation, not persistence")
            }
            Err(_) => {
                tracing::warn!("[{}] The persistence of the stream is gone", self.actor.stream_id());
                false
            }
        }
    }

    async fn run(
        &mut self,
        persistence_result: &mut Option<oneshot::Receiver<PersistResult>>,
        receiver: &mut mpsc::UnboundedReceiver<Message>,
    ) {
        let mut backstop = time::interval(self.config.backstop);
        backstop.set_missed_tick_behavior(MissedTickBehavior::Delay);
        backstop.tick().await;
        let wake = self.wake.clone();
        let mut next_wake_pass = Instant::now();

        // Commands that arrived while loading are worked at once.
        let mut alive = self.pass(persistence_result).await;
        while alive {
            let deadline = self.grace_deadline;
            let reconcile_deadline = self.reconcile_deadline;
            let grace = async {
                match deadline {
                    Some(deadline) => time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                biased;
                message = receiver.recv() => match message {
                    Some(Message::Subscribe(connection_id)) => self.subscribe(connection_id).await,
                    Some(Message::Unsubscribe(connection_id)) => self.unsubscribe(connection_id),
                    Some(Message::Stop) | None => {
                        self.drain(persistence_result).await;
                        alive = false;
                    }
                },
                result = async {
                    persistence_result
                        .as_mut()
                        .expect("the result branch is disabled without an outstanding job")
                        .await
                }, if persistence_result.is_some() => match result {
                    Ok(result) => {
                        persistence_result.take();
                        alive = self.settle(result, persistence_result).await;
                    }
                    // The persistence is gone, so nothing can be made durable any more.
                    Err(_) => {
                        persistence_result.take();
                        self.actor.give_up().await;
                        alive = false;
                    }
                },
                _ = wake.notified() => {
                    time::sleep_until(next_wake_pass).await;
                    alive = self.pass(persistence_result).await;
                    next_wake_pass = Instant::now() + self.config.min_wake_spacing;
                }
                _ = backstop.tick() => alive = self.pass(persistence_result).await,
                _ = time::sleep_until(reconcile_deadline), if persistence_result.is_none() => {
                    alive = self.reconcile(persistence_result).await;
                }
                _ = grace => {
                    // The last words: whatever is waiting in the inbox is made durable.
                    self.drain(persistence_result).await;
                    alive = false;
                }
            }
        }
    }

    /// One look at the inbox: whatever waits there is versioned, sent to the subscribers and submitted.
    /// Returns whether the stream goes on.
    async fn pass(&mut self, persistence_result: &mut Option<oneshot::Receiver<PersistResult>>) -> bool {
        let ingested = match self.actor.ingest().await {
            Ok(ingested) => {
                self.failures = 0;
                ingested
            }
            Err(err) => return self.failed(err).await,
        };
        // Logged on the edges only: a saturated stream passes on every persistence round-trip.
        if ingested.throttled != self.saturated {
            self.saturated = ingested.throttled;
            if self.saturated {
                tracing::warn!(
                    "[{}] The optimistic backlog is full, the inbox is left to wait for persistence",
                    self.actor.stream_id()
                );
            } else {
                tracing::info!("[{}] The optimistic backlog has room again", self.actor.stream_id());
            }
        }
        if !ingested.commands.is_empty() {
            self.actor.deliver(&ingested.commands).await;
            self.arm_grace_if_empty();
        }
        self.submit(persistence_result).await
    }

    /// Submits what is waiting, if the persistence is free. Returns whether the stream goes on.
    async fn submit(&mut self, persistence_result: &mut Option<oneshot::Receiver<PersistResult>>) -> bool {
        match self.try_submit(persistence_result).await {
            Ok(_) => true,
            // The persistence is gone: what the subscribers were told can never be confirmed.
            Err(_) => {
                tracing::warn!("[{}] The persistence of the stream is gone", self.actor.stream_id());
                self.actor.give_up().await;
                false
            }
        }
    }

    /// Hands the waiting batch to the persistence unless one is already on its way. The single place
    /// that opens the in-flight window, so there is one place that can leave two jobs outstanding.
    /// Returns whether a job was sent.
    async fn try_submit(
        &mut self,
        persistence_result: &mut Option<oneshot::Receiver<PersistResult>>,
    ) -> Result<bool, Arc<[StoredCommand]>> {
        if persistence_result.is_some() {
            return Ok(false);
        }
        if self.actor.optimistic().is_empty() {
            return Ok(false);
        }
        let commands = self.actor.optimistic().to_vec().into();
        let (baseline_version, baseline_chain_hash) = self.actor.confirmed_head();
        *persistence_result = Some(
            self.request_persist(baseline_version, baseline_chain_hash, commands)
                .await?,
        );
        self.reconcile_deadline = Instant::now() + self.config.backstop;
        Ok(true)
    }

    /// Checks the durable head after a quiet period. Sending any work rearms the deadline, so an active
    /// stream is reconciled by its normal persistence results instead of additional head reads.
    async fn reconcile(&mut self, persistence_result: &mut Option<oneshot::Receiver<PersistResult>>) -> bool {
        let (version, chain_hash) = self.actor.confirmed_head();
        match self.request_reconciliation(version, chain_hash).await {
            Ok(result) => {
                *persistence_result = Some(result);
                self.reconcile_deadline = Instant::now() + self.config.backstop;
                true
            }
            Err(()) => {
                tracing::warn!("[{}] The persistence of the stream is gone", self.actor.stream_id());
                self.actor.give_up().await;
                false
            }
        }
    }

    /// An answer of the persistence. Returns whether the stream goes on.
    async fn settle(
        &mut self,
        result: PersistResult,
        persistence_result: &mut Option<oneshot::Receiver<PersistResult>>,
    ) -> bool {
        match result {
            PersistResult::Current => {
                self.failures = 0;
                true
            }
            PersistResult::Advanced {
                canonical,
                snapshot,
                checkpoint,
            } => {
                self.failures = 0;
                if self.actor.accept_canonical(&canonical, snapshot) {
                    // The commands that piled up behind the batch are at other versions than the
                    // subscribers were told: what they have of the stream is void.
                    self.actor.resync_all().await;
                }
                if let Some(checkpoint) = checkpoint {
                    self.actor.broadcast_checkpoint(checkpoint).await;
                }
                self.arm_grace_if_empty();
                // The backlog may have been held back while the batch was in flight.
                self.wake.notify_one();
                self.submit(persistence_result).await
            }
            PersistResult::Reloaded {
                snapshot,
                tail,
                stored_inbox,
                reason,
            } => {
                tracing::info!("[{}] The stream is reloaded: {reason}", self.actor.stream_id());
                self.failures = 0;
                self.actor.accept_reload(snapshot, tail.to_vec(), stored_inbox);
                // What the subscribers were told did not survive: they start again from the log.
                self.actor.resync_all().await;
                self.arm_grace_if_empty();
                self.wake.notify_one();
                true
            }
            PersistResult::Failed { error } => self.failed(error).await,
        }
    }

    /// Makes durable what is waiting and waits for the answer, so that once the stream has ended, every
    /// command it took from the inbox is in the log. A command that arrives after this is left in the
    /// inbox for whoever serves the stream next.
    async fn drain(&mut self, persistence_result: &mut Option<oneshot::Receiver<PersistResult>>) {
        if let Err(err) = self.actor.ingest().await {
            tracing::warn!(
                "[{}] The last look at the inbox failed: {err:#?}",
                self.actor.stream_id()
            );
        }
        loop {
            match self.try_submit(persistence_result).await {
                // Nothing is waiting and nothing is in flight: everything taken is durable.
                Ok(false) if persistence_result.is_none() => return,
                Ok(_) => {}
                Err(_) => return,
            }
            let Some(result) = persistence_result.take() else {
                return;
            };
            let Ok(result) = result.await else {
                return;
            };
            match result {
                PersistResult::Current => continue,
                PersistResult::Advanced { canonical, snapshot, .. } => {
                    self.actor.accept_canonical(&canonical, snapshot);
                }
                PersistResult::Reloaded {
                    snapshot, tail, stored_inbox, ..
                } => {
                    // The log holds a history of its own; the commands of this instance that are not in
                    // it stay in the inbox, so they are not lost by giving up here.
                    self.actor.accept_reload(snapshot, tail.to_vec(), stored_inbox);
                    return;
                }
                PersistResult::Failed { error, .. } => {
                    tracing::warn!(
                        "[{}] The last commands could not be stored: {error:#?}",
                        self.actor.stream_id()
                    );
                    return;
                }
            }
        }
    }

    async fn failed(&mut self, err: impl std::fmt::Debug) -> bool {
        self.failures += 1;
        tracing::warn!(
            "[{}] The pass failed ({} in a row): {err:#?}",
            self.actor.stream_id(),
            self.failures
        );
        if self.failures >= self.config.max_failures {
            self.actor.give_up().await;
            false
        } else {
            true
        }
    }

    async fn subscribe(&mut self, connection_id: Uuid) {
        self.grace_deadline = None;
        self.actor.subscribe(connection_id, 0);
        self.actor.resync(connection_id).await;
        self.arm_grace_if_empty();
    }

    fn unsubscribe(&mut self, connection_id: Uuid) {
        self.actor.unsubscribe(connection_id);
        self.arm_grace_if_empty();
    }

    fn arm_grace_if_empty(&mut self) {
        if self.actor.subscriber_count() == 0 && self.grace_deadline.is_none() {
            self.grace_deadline = Some(Instant::now() + self.config.unload_grace);
        }
    }
}
