use crate::services::hub::hub_metrics::hub_metrics;
use crate::{
    models::{
        messages::{DisconnectReason, HubEvent, HubMessage, TopicKey},
        HubError,
    },
    repositories::hub_registry::{
        redis::{RedisHubConnectionDb, RegistryChange},
        HubConnection, HubConnectionDb, HubRegistry,
    },
    services::{
        hub::{
            hub_command::ControlCommand,
            hub_connections::{Connections, DeadConnection, RemovedConnection},
            hub_subscribers::Subscribers,
            tasks::{ChatDispatcher, Heartbeat, SessionChecker},
        },
        ChatService,
    },
};
use futures::{FutureExt, StreamExt};
use opentelemetry::trace::SpanContext;
use shine_infra::session::{CurrentUserService, SessionKey};
use shine_infra::telemetry::{current_span_context, label, link_span};
use std::{
    any::Any,
    collections::{HashMap, HashSet, VecDeque},
    future::Future,
    panic::AssertUnwindSafe,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::{info_span, Instrument};
use uuid::Uuid;

/// Max time of one registry operation, including the wait for a pooled Redis connection. Every
/// registry operation is a single atomic step, so abandoning one midway is safe.
const REGISTRY_OP_TIMEOUT: Duration = Duration::from_secs(2);

/// Commands waiting for the command loop above which new connections are refused: a stalled loop
/// must not accumulate an unbounded backlog of connects.
const MAX_PENDING_COMMANDS: usize = 10_000;

/// Min time between two connection attempts of a user.
const CONNECT_MIN_INTERVAL: Duration = Duration::from_millis(500);

/// Connect-attempt records kept before expired ones are pruned.
const CONNECT_RECORDS_PRUNE_AT: usize = 10_000;

/// Total time the shutdown spends on registry cleanup. Entries not removed in time expire by TTL.
const SHUTDOWN_REGISTRY_BUDGET: Duration = Duration::from_secs(10);

/// Registry removals in flight during shutdown.
const SHUTDOWN_REGISTRY_CONCURRENCY: usize = 8;

/// Delays between attempts to (re)subscribe to the registry channel.
const LISTEN_RETRY_MIN: Duration = Duration::from_secs(1);
const LISTEN_RETRY_MAX: Duration = Duration::from_secs(30);

/// Cadence configuration for the hub's periodic connection consumers.
pub struct HubIntervals {
    pub heartbeat: Duration,
    pub session_check: Duration,
    pub chat: Duration,
}

/// Point-in-time state of the hub, for status reporting.
#[derive(Clone, Debug)]
pub struct HubStats {
    pub connections: usize,
    pub subscribers: usize,
    /// Commands waiting for the command loop.
    pub pending_commands: usize,
    /// The command loop is running.
    pub running: bool,
    /// Background tasks that ended while the hub is running.
    pub stopped_tasks: Vec<&'static str>,
}

type ControlMessage = (ControlCommand, SpanContext, Instant);

struct Inner {
    control_tx: mpsc::UnboundedSender<ControlMessage>,
    pending_commands: AtomicUsize,
    connections: Connections,
    subscribers: Subscribers,
    hub_registry: RedisHubConnectionDb,
    chat_service: ChatService,
    /// Connections with a queued `DropConnection`, so a stalled loop is not fed duplicates.
    pending_drops: Mutex<HashSet<Uuid>>,
    /// Last connection attempt of recently seen users.
    connect_attempts: Mutex<HashMap<Uuid, Instant>>,
    /// Background tasks, to report the ones that died.
    tasks: Mutex<Vec<(&'static str, JoinHandle<()>)>>,
}

/// Messaging service for connected users.
#[derive(Clone)]
pub struct HubService {
    inner: Arc<Inner>,
}

/// Best-effort text of a panic payload.
pub(in crate::services::hub) fn panic_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string panic payload")
}

impl HubService {
    pub async fn new(
        hub_registry: RedisHubConnectionDb,
        session_service: Arc<CurrentUserService>,
        chat_service: ChatService,
        intervals: HubIntervals,
    ) -> Self {
        let (control_tx, control_rx) = mpsc::unbounded_channel();

        let service = Self {
            inner: Arc::new(Inner {
                control_tx,
                pending_commands: AtomicUsize::new(0),
                connections: Connections::new(),
                subscribers: Subscribers::new(),
                hub_registry,
                chat_service: chat_service.clone(),
                pending_drops: Mutex::new(HashSet::new()),
                connect_attempts: Mutex::new(HashMap::new()),
                tasks: Mutex::new(Vec::new()),
            }),
        };

        // All consumer subscriptions must be registered before the dispatcher starts: the
        // consumers are event-sourced, so a lifecycle event dispatched before they subscribe is
        // lost to them forever. The start_* calls subscribe synchronously (awaited), and the
        // dispatcher is started last so nothing drains the channels until every subscription is in
        // place.
        let mut tasks = Vec::new();
        tasks.extend(
            Self::start_registry_listener(service.clone())
                .await
                .map(|task| ("registry listener", task)),
        );
        tasks.push(("heartbeat", Heartbeat::start(service.clone(), intervals.heartbeat).await));
        tasks.push((
            "session checker",
            SessionChecker::start(service.clone(), session_service, intervals.session_check).await,
        ));
        tasks.push((
            "chat dispatcher",
            ChatDispatcher::start(service.clone(), chat_service, intervals.chat).await,
        ));
        service.inner.tasks.lock().unwrap().extend(tasks);

        Self::start_dispatcher(service.clone(), control_rx);
        service
    }

    fn send_control(&self, command: ControlCommand) -> Result<(), HubError> {
        self.inner.pending_commands.fetch_add(1, Ordering::Relaxed);
        hub_metrics().command_queue_depth.add(1, &[]);
        self.inner
            .control_tx
            .send((command, current_span_context(), Instant::now()))
            .map_err(|_| {
                self.command_done();
                HubError::SendCommandFailed
            })
    }

    /// A command left the queue.
    fn command_done(&self) {
        self.inner.pending_commands.fetch_sub(1, Ordering::Relaxed);
        hub_metrics().command_queue_depth.add(-1, &[]);
    }

    /// Runs a registry operation within [`REGISTRY_OP_TIMEOUT`], counting failures.
    async fn registry<T>(&self, op: &'static str, work: impl Future<Output = Result<T, HubError>>) -> Result<T, HubError> {
        let result = match tokio::time::timeout(REGISTRY_OP_TIMEOUT, work).await {
            Ok(result) => result,
            Err(_) => Err(HubError::RegistryTimeout),
        };
        if let Err(err) = &result {
            let kind = if matches!(err, HubError::RegistryTimeout) { "timeout" } else { "error" };
            hub_metrics()
                .registry_failures
                .add(1, &[label("op", op), label("kind", kind)]);
        }
        result
    }

    /// Logs and counts a connection removed by the hub, and tells its socket why.
    fn record_drop(&self, removed: RemovedConnection, reason: DisconnectReason) -> (Uuid, Uuid) {
        let (user_id, connection_id) = (removed.user_id, removed.connection_id);
        tracing::info!("[{user_id}] [{connection_id}] Connection dropped: {reason}");
        hub_metrics()
            .connections_dropped
            .add(1, &[label("reason", reason.as_str())]);
        removed.notify(HubEvent::UserDisconnected { user_id, connection_id, reason });
        (user_id, connection_id)
    }

    /// Prunes any connection found dead during an egress send.
    fn drop_dead(&self, dead: impl IntoIterator<Item = DeadConnection>) -> Result<(), HubError> {
        for DeadConnection { connection_id, reason } in dead {
            if !self.inner.pending_drops.lock().unwrap().insert(connection_id) {
                continue;
            }
            if let Err(err) = self.send_control(ControlCommand::DropConnection { connection_id, reason }) {
                self.inner.pending_drops.lock().unwrap().remove(&connection_id);
                return Err(err);
            }
        }
        Ok(())
    }

    /// Admission check of a connection attempt, before the socket is upgraded: refuses a user
    /// reconnecting faster than [`CONNECT_MIN_INTERVAL`] and any attempt while the command loop is
    /// far behind.
    pub fn check_connect_allowed(&self, user_id: Uuid) -> Result<(), HubError> {
        if self.inner.pending_commands.load(Ordering::Relaxed) >= MAX_PENDING_COMMANDS {
            hub_metrics().connects_rejected.add(1, &[label("reason", "overloaded")]);
            return Err(HubError::Overloaded);
        }

        let now = Instant::now();
        let mut attempts = self.inner.connect_attempts.lock().unwrap();
        if attempts.len() >= CONNECT_RECORDS_PRUNE_AT {
            attempts.retain(|_, last| now.duration_since(*last) < CONNECT_MIN_INTERVAL);
        }
        match attempts.insert(user_id, now) {
            Some(last) if now.duration_since(last) < CONNECT_MIN_INTERVAL => {
                hub_metrics().connects_rejected.add(1, &[label("reason", "throttled")]);
                Err(HubError::TooManyConnects)
            }
            _ => Ok(()),
        }
    }

    /// Requests a new connection for a user, superseding any prior one, and returns its id.
    pub fn request_connection(
        &self,
        user_id: Uuid,
        session_key: SessionKey,
        tx: mpsc::Sender<HubMessage>,
        topics: Vec<TopicKey>,
    ) -> Result<Uuid, HubError> {
        if self.inner.pending_commands.load(Ordering::Relaxed) >= MAX_PENDING_COMMANDS {
            hub_metrics().connects_rejected.add(1, &[label("reason", "overloaded")]);
            return Err(HubError::Overloaded);
        }

        let connection_id = Uuid::new_v4();
        self.send_control(ControlCommand::ConnectUser {
            user_id,
            connection_id,
            session_key,
            tx,
            topics,
        })?;
        Ok(connection_id)
    }

    /// Requests removal of a connection, only if `connection_id` still matches the user's active one.
    pub fn request_disconnection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        self.send_control(ControlCommand::DisconnectUser {
            user_id,
            connection_id,
            reason: DisconnectReason::Client,
        })
    }

    /// Like [`Self::request_disconnection`], for a disconnect forced by the hub; counted as a dropped
    /// connection with the given reason once the connection is actually removed.
    pub fn request_drop(&self, user_id: Uuid, connection_id: Uuid, reason: DisconnectReason) -> Result<(), HubError> {
        debug_assert!(reason.is_drop(), "a client disconnect is not a drop");
        self.send_control(ControlCommand::DisconnectUser { user_id, connection_id, reason })
    }

    /// Requests hub shutdown.
    pub fn request_shutdown(&self) -> Result<(), HubError> {
        self.send_control(ControlCommand::Shutdown)
    }

    /// Completes once the command loop has ended, i.e. after a shutdown has dropped every connection.
    #[cfg(test)]
    pub async fn wait_stopped(&self) {
        self.inner.control_tx.closed().await;
    }

    /// Requests a recheck of every local connection against the registry, for when change notices
    /// may have been missed.
    pub fn request_registry_resync(&self) -> Result<(), HubError> {
        self.send_control(ControlCommand::ResyncRegistry)
    }

    /// Notifies the hub that a user's registry entry changed, prompting an active-connection recheck.
    pub fn notify_registry_changed(&self, user_id: Uuid) -> Result<(), HubError> {
        self.send_control(ControlCommand::HubRegistryChanged { user_id })
    }

    /// Sends an egress message to a specific connection. Returns whether the message was queued for delivery.
    #[tracing::instrument(level = "debug", skip_all, fields(ws.connection_id = %connection_id))]
    pub async fn send_to_connection(&self, connection_id: Uuid, message: HubMessage) -> Result<bool, HubError> {
        match self.inner.connections.send_to_connection(connection_id, message).await {
            Ok(queued) => Ok(queued),
            Err(dead) => self.drop_dead([dead]).map(|()| false),
        }
    }

    /// Sends an egress message to a user's active connection.
    #[allow(dead_code)] // part of the hub egress API; not yet wired to a caller
    #[tracing::instrument(level = "debug", skip_all, fields(enduser.id = %user_id))]
    pub async fn send_to_user(&self, user_id: Uuid, message: HubMessage) -> Result<(), HubError> {
        self.drop_dead(self.inner.connections.send_to_user(user_id, message).await.err())
    }

    /// Broadcasts an egress message to every connection subscribed to its topic.
    #[allow(dead_code)] // part of the hub egress API; not yet wired to a caller
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn broadcast(&self, message: HubMessage) -> Result<(), HubError> {
        self.drop_dead(
            self.inner
                .connections
                .broadcast(message)
                .await
                .err()
                .unwrap_or_default(),
        )
    }

    /// Subscribes to a set of topics on an unbounded, lossless channel.
    pub async fn subscribe(&self, topics: Vec<TopicKey>) -> mpsc::UnboundedReceiver<HubMessage> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.inner.subscribers.subscribe(topics, tx).await;
        rx
    }

    /// Live state of the hub for status reporting.
    pub async fn stats(&self) -> HubStats {
        let connections = self.inner.connections.len().await;
        let subscribers = self.inner.subscribers.len().await;
        let running = !self.inner.control_tx.is_closed();
        let stopped_tasks = if running {
            self.inner
                .tasks
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, task)| task.is_finished())
                .map(|(name, _)| *name)
                .collect()
        } else {
            Vec::new()
        };
        HubStats {
            connections,
            subscribers,
            pending_commands: self.inner.pending_commands.load(Ordering::Relaxed),
            running,
            stopped_tasks,
        }
    }

    /// Starts the command loop, the sole writer of connection state.
    fn start_dispatcher(service: HubService, mut control_rx: mpsc::UnboundedReceiver<ControlMessage>) {
        tokio::spawn(async move {
            while let Some((command, origin, queued_at)) = control_rx.recv().await {
                service.command_done();
                let name = command.name();
                let metrics = hub_metrics();
                metrics
                    .command_queue_delay
                    .record(queued_at.elapsed().as_secs_f64(), &[label("command", name)]);

                // Linked, not parented: the queued command must not keep the originating request span open.
                let span = info_span!(
                    parent: None,
                    "hub command",
                    hub.command = name,
                    enduser.id = command.user_id().map(display),
                    ws.connection_id = command.connection_id().map(display)
                );
                link_span(&span, origin);
                let start = Instant::now();
                // A panic in one command must not take the whole hub down with it.
                let outcome = AssertUnwindSafe(service.process(command).instrument(span))
                    .catch_unwind()
                    .await;
                let keep_running = match outcome {
                    Ok(keep_running) => keep_running,
                    Err(payload) => {
                        tracing::error!("Hub command {name} panicked: {}", panic_message(payload.as_ref()));
                        metrics.task_panics.add(1, &[label("task", "command loop")]);
                        true
                    }
                };
                metrics.commands.add(1, &[label("command", name)]);
                metrics
                    .command_duration
                    .record(start.elapsed().as_secs_f64(), &[label("command", name)]);
                if !keep_running {
                    break;
                }
            }
            tracing::info!("Hub command loop stopped");
            let tasks = std::mem::take(&mut *service.inner.tasks.lock().unwrap());
            for (_, task) in tasks {
                task.abort();
            }
        });
    }

    /// Subscribes to the registry channel. Returns the task retrying the subscription when the first
    /// attempt failed, so a takeover is never left to the periodic heartbeat alone.
    async fn start_registry_listener(service: HubService) -> Option<JoinHandle<()>> {
        match service.listen_to_registry().await {
            Ok(()) => None,
            Err(err) => {
                tracing::error!("Failed to listen to hub registry changes, retrying: {err:#?}");
                Some(tokio::spawn(async move {
                    let mut delay = LISTEN_RETRY_MIN;
                    loop {
                        tokio::time::sleep(delay).await;
                        match service.listen_to_registry().await {
                            Ok(()) => {
                                tracing::info!("Listening to hub registry changes");
                                // Notices published before the subscription are lost.
                                let _ = service.request_registry_resync();
                                return;
                            }
                            Err(err) => {
                                tracing::warn!("Failed to listen to hub registry changes: {err:#?}");
                                delay = (delay * 2).min(LISTEN_RETRY_MAX);
                            }
                        }
                    }
                }))
            }
        }
    }

    async fn listen_to_registry(&self) -> Result<(), HubError> {
        let notifier = self.clone();
        self.inner
            .hub_registry
            .listen_to_registry_changes(move |change| match change {
                RegistryChange::User(user_id) => {
                    // Notices about users this instance does not hold are the common case; they
                    // need no command, no span and no registry read.
                    if !notifier.inner.connections.may_hold_user(user_id) {
                        return;
                    }
                    if let Err(err) = notifier.notify_registry_changed(user_id) {
                        tracing::warn!("[{user_id}] Failed to enqueue hub registry change command: {err:#?}");
                    }
                }
                RegistryChange::Resync => {
                    if let Err(err) = notifier.request_registry_resync() {
                        tracing::warn!("Failed to enqueue hub registry resync command: {err:#?}");
                    }
                }
            })
            .await
    }

    /// Processes one control command. Returns if the dispatcher should continue (true) or exit (false).
    async fn process(&self, command: ControlCommand) -> bool {
        tracing::debug!("Processing command: {command:#?}");
        match command {
            ControlCommand::ConnectUser {
                user_id,
                connection_id,
                session_key,
                tx,
                topics,
            } => {
                // Record the replacement in the registry first. If this fails, the previously
                // active connection must stay intact and tracked, so return before tearing it down
                // — otherwise a transient Redis error would leave the user with no connection and
                // an orphaned, untracked new socket.
                if let Err(err) = self.create_registry_connection(user_id, connection_id).await {
                    // The write is not atomic with its reply: it may have landed although the call
                    // failed (e.g. a lost response). Then the registry already names the new
                    // connection, and keeping the old one would only get it dropped as inactive.
                    if !self.is_registry_connection(user_id, connection_id).await {
                        tracing::error!("[{user_id}] Failed to create hub connection: {err:#?}");
                        return true;
                    }
                    tracing::warn!("[{user_id}] Hub connection registered despite an error: {err:#?}");
                }

                // Now that the new connection is durably registered, tear down any different
                // connection that was active for this user, so the old socket closes and the
                // connection tracker drops it. Same-id re-registration is not a replacement.
                if let Some((old_connection_id, _)) = self.inner.connections.find_active(user_id).await {
                    if old_connection_id != connection_id {
                        if let Some(removed) = self
                            .inner
                            .connections
                            .remove_connection(user_id, Some(old_connection_id))
                            .await
                        {
                            let (user_id, connection_id) = self.record_drop(removed, DisconnectReason::Replaced);
                            self.publish(HubEvent::UserDisconnected {
                                user_id,
                                connection_id,
                                reason: DisconnectReason::Replaced,
                            })
                            .await;
                        }
                    }
                }

                self.inner
                    .connections
                    .register_connection(user_id, connection_id, session_key, tx, topics)
                    .await;
                self.publish(HubEvent::UserConnected {
                    user_id,
                    connection_id,
                    session_key,
                })
                .await;
                true
            }
            ControlCommand::DisconnectUser { user_id, connection_id, reason } => {
                // Only removes the entry when `connection_id` still matches the active
                // connection, so a stale disconnect can never tear down a fresh reconnect.
                let Some(removed) = self
                    .inner
                    .connections
                    .remove_connection(user_id, Some(connection_id))
                    .await
                else {
                    return true;
                };

                // A client-requested disconnect is not a drop; its socket is already closing.
                let removed_connection_id = if reason.is_drop() {
                    self.record_drop(removed, reason).1
                } else {
                    removed.connection_id
                };
                if let Err(err) = self.remove_registry_connection(user_id, removed_connection_id).await {
                    // Best-effort: publish UserDisconnected regardless so the socket and trackers
                    // never wedge, and let the registry TTL clean up the stale row.
                    tracing::error!("[{user_id}] Failed to remove hub connection (relying on TTL cleanup): {err:#?}");
                }

                self.publish(HubEvent::UserDisconnected {
                    user_id,
                    connection_id: removed_connection_id,
                    reason,
                })
                .await;
                true
            }
            ControlCommand::DropConnection { connection_id, reason } => {
                self.inner.pending_drops.lock().unwrap().remove(&connection_id);
                // A producer found this connection's channel dead during an off-loop send. Prune it
                // here so all mutation stays on the loop. Force-remove by id (a fresh reconnect that
                // replaced it has a different id and its channel would not be dead).
                if let Some(removed) = self.inner.connections.remove_connection_by_id(connection_id).await {
                    let (user_id, connection_id) = self.record_drop(removed, reason);
                    if let Err(err) = self.remove_registry_connection(user_id, connection_id).await {
                        tracing::error!("[{user_id}] Failed to remove dropped connection from registry (TTL will clean up): {err:#?}");
                    }
                    self.publish(HubEvent::UserDisconnected { user_id, connection_id, reason })
                        .await;
                }
                true
            }
            ControlCommand::HubRegistryChanged { user_id } => {
                self.process_registry_change(user_id).await;
                true
            }
            ControlCommand::ResyncRegistry => {
                self.resync_registry().await;
                true
            }
            ControlCommand::Shutdown => {
                self.shutdown().await;
                false
            }
        }
    }

    async fn create_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        self.registry("create", async {
            let mut context = self.inner.hub_registry.create_context().await?;
            context.create_connection(user_id, connection_id).await
        })
        .await
    }

    /// Whether the registry holds `connection_id` as the user's active connection; `false` on a read error.
    async fn is_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> bool {
        let found = self
            .registry("find", async {
                let mut context = self.inner.hub_registry.create_context().await?;
                context.find_connection_by_user(user_id).await
            })
            .await;
        match found {
            Ok(found) => found.is_some_and(|found| found.connection_id == connection_id),
            Err(err) => {
                tracing::warn!("[{user_id}] Failed to read hub connection: {err:#?}");
                false
            }
        }
    }

    async fn remove_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        self.registry("remove", async {
            let mut context = self.inner.hub_registry.create_context().await?;
            context.remove_connection_if_active(user_id, connection_id).await?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn heartbeat_registry_connection(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> Result<bool, HubError> {
        self.registry("heartbeat", async {
            let mut context = self.inner.hub_registry.create_context().await?;
            context.heartbeat_connection(user_id, connection_id).await
        })
        .await
    }

    /// Batched heartbeat over the given connections, returning the ones the registry no longer holds
    /// as active. See [`HubRegistry::heartbeat_connections`] for the reconciliation semantics.
    pub(crate) async fn heartbeat_registry_connections(
        &self,
        connections: &[HubConnection],
    ) -> Result<Vec<HubConnection>, HubError> {
        // The whole batch gets a budget that grows with its size: one timeout per chunk would not
        // bound it, and a fixed one would fail large batches.
        let chunks = connections.len().div_ceil(256).max(1) as u32;
        match tokio::time::timeout(REGISTRY_OP_TIMEOUT * chunks, async {
            let mut context = self.inner.hub_registry.create_context().await?;
            context.heartbeat_connections(connections).await
        })
        .await
        {
            Ok(result) => result,
            Err(_) => Err(HubError::RegistryTimeout),
        }
        .inspect_err(|_| {
            hub_metrics()
                .registry_failures
                .add(1, &[label("op", "heartbeat_batch"), label("kind", "error")]);
        })
    }

    async fn process_registry_change(&self, user_id: Uuid) {
        let Some((connection_id, _session_key)) = self.inner.connections.find_active(user_id).await else {
            return;
        };

        let is_active = match self.heartbeat_registry_connection(user_id, connection_id).await {
            Ok(is_active) => is_active,
            Err(err) => {
                tracing::error!("[{user_id}] Failed to heartbeat hub connection: {err:#?}");
                return;
            }
        };

        if !is_active {
            self.drop_registry_inactive(user_id, connection_id).await;
        }
    }

    /// Rechecks every local connection against the registry, after change notices may have been missed.
    async fn resync_registry(&self) {
        let connections: Vec<HubConnection> = self
            .inner
            .connections
            .snapshot()
            .await
            .into_iter()
            .map(|(user_id, connection_id)| HubConnection { user_id, connection_id })
            .collect();
        if connections.is_empty() {
            return;
        }

        let stale = match self.heartbeat_registry_connections(&connections).await {
            Ok(stale) => stale,
            Err(err) => {
                tracing::error!("Failed to resync hub connections with the registry: {err:#?}");
                return;
            }
        };
        tracing::info!("Hub registry resync: {} of {} connections are stale", stale.len(), connections.len());
        for HubConnection { user_id, connection_id } in stale {
            self.drop_registry_inactive(user_id, connection_id).await;
        }
    }

    /// Removes a connection another instance took over. If a fresh reconnect replaced it in the
    /// meantime, the connection ids differ and the new connection is left untouched.
    async fn drop_registry_inactive(&self, user_id: Uuid, connection_id: Uuid) {
        let Some(removed) = self
            .inner
            .connections
            .remove_connection(user_id, Some(connection_id))
            .await
        else {
            return;
        };

        let (user_id, connection_id) = self.record_drop(removed, DisconnectReason::RegistryInactive);
        self.publish(HubEvent::UserDisconnected {
            user_id,
            connection_id,
            reason: DisconnectReason::RegistryInactive,
        })
        .await;
    }

    /// Stops the internal consumers, tells every socket the hub is going away, then cleans the
    /// registry within a time budget. Nothing is published after the Shutdown event: the consumers
    /// are gone, and the sockets are told directly.
    async fn shutdown(&self) {
        self.inner
            .subscribers
            .publish(HubMessage::Hub(HubEvent::Shutdown))
            .await;
        self.inner.subscribers.clear().await;

        let unlisten = async {
            if let Err(err) = self.inner.hub_registry.unlisten_registry_changes().await {
                tracing::warn!("Failed to stop listening to hub registry changes: {err:#?}");
            }
            if let Err(err) = self.inner.chat_service.unlisten_comments().await {
                tracing::warn!("Failed to stop listening to chat appends: {err:#?}");
            }
        };
        if tokio::time::timeout(REGISTRY_OP_TIMEOUT, unlisten).await.is_err() {
            tracing::warn!("Timed out stopping the hub listeners");
        }

        // Sockets first: they close as soon as they are told, whatever Redis is doing.
        let dropped: Vec<(Uuid, Uuid)> = self
            .inner
            .connections
            .remove_all()
            .await
            .into_iter()
            .map(|removed| self.record_drop(removed, DisconnectReason::Shutdown))
            .collect();

        let cleanup = futures::stream::iter(dropped).for_each_concurrent(SHUTDOWN_REGISTRY_CONCURRENCY, |(user_id, connection_id)| async move {
            if let Err(err) = self.remove_registry_connection(user_id, connection_id).await {
                tracing::warn!("[{user_id}] Failed to remove hub connection on shutdown (TTL will clean up): {err:#?}");
            }
        });
        if tokio::time::timeout(SHUTDOWN_REGISTRY_BUDGET, cleanup).await.is_err() {
            tracing::warn!("Registry cleanup on shutdown timed out; the remaining entries expire by TTL");
        }
    }

    /// Delivers a lifecycle `HubEvent` to every internal subscriber and every topic-matching
    /// connection, pruning any connection found dead.
    async fn publish(&self, event: HubEvent) {
        let mut pending = VecDeque::from([event]);
        while let Some(event) = pending.pop_front() {
            let message = HubMessage::Hub(event);
            // Internal subscribers first: only the loop delivers to them.
            self.inner.subscribers.publish(message.clone()).await;

            if let Err(dead) = self.inner.connections.broadcast(message).await {
                for DeadConnection { connection_id, reason } in dead {
                    if let Some(removed) = self.inner.connections.remove_connection_by_id(connection_id).await {
                        let (user_id, connection_id) = self.record_drop(removed, reason);
                        if let Err(err) = self.remove_registry_connection(user_id, connection_id).await {
                            tracing::error!(
                                "[{user_id}] Failed to remove dead connection from registry (TTL will clean up): {err:#?}"
                            );
                        }
                        pending.push_back(HubEvent::UserDisconnected { user_id, connection_id, reason });
                    }
                }
            }
        }
    }
}
