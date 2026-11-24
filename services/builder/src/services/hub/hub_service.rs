use crate::{
    models::{
        messages::{
            CommandFailure, DisconnectReason, HubEvent, HubMessage, StreamEvent, StreamMessage, SubscribeFailure,
            TopicKey,
        },
        CommandStreamError, HubError,
    },
    repositories::{
        command_inbox::redis::RedisCommandInboxDb,
        command_log::pg::PgCommandLogDb,
        hub_registry::{redis::RedisHubConnectionDb, HubConnection, HubConnectionDb, HubRegistry},
    },
    services::{
        command_stream::{stream_metrics, StreamRegistry},
        hub::{
            hub_command::ControlCommand,
            hub_connections::{Connections, DeadConnection, RemovedConnection},
            hub_control_queue::{ControlMessage, ControlQueue},
            hub_metrics::hub_metrics,
            hub_stream_egress::HubStreamEgress,
            hub_subscribers::Subscribers,
            socket_channel::SocketSender,
            tasks::{Heartbeat, SessionChecker, StreamDispatcher},
        },
        StreamEgress,
    },
};
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use shine_core::tokio::TaskSupervisor;
use shine_game::replica::StreamId;
use shine_infra::{
    serde::{consts, duration},
    session::{CurrentUserService, SessionKey},
    sync::{Admission, AdmissionError, AdmissionPermit, RateLimit},
    telemetry::link_span,
};
use std::collections::VecDeque;
use std::time::Instant;
use std::{sync::Arc, time::Duration};
use thiserror::Error as ThisError;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};
use tracing::{info_span, Instrument};
use uuid::Uuid;
use validator::Validate;

/// Point-in-time counts of hub state, for status reporting.
#[derive(Clone, Debug)]
pub struct HubStats {
    pub connections: usize,
    pub subscribers: usize,
}

/// Configuration for the hub's periodic connection consumers and its load limits.
#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct HubConfig {
    #[serde(default = "duration::seconds::<300>", with = "duration::seconds")]
    pub heartbeat: Duration,
    #[serde(default = "duration::seconds::<900>", with = "duration::seconds")]
    pub session_check: Duration,
    #[serde(default = "duration::seconds::<10>", with = "duration::seconds")]
    pub stream_shutdown_timeout: Duration,
    #[serde(default = "consts::bool::<false>")]
    pub ping_enabled: bool,
    /// Sockets served at the same time, counted from the upgrade to the close.
    #[serde(default = "consts::usize::<10000>")]
    #[validate(range(min = 1, max = 1_000_000))]
    pub max_connections: usize,
    /// Connects of one user.
    #[serde(default = "HubConfig::default_connect_rate")]
    #[validate(nested)]
    pub connect_rate: RateLimit,
}

impl HubConfig {
    const fn default_connect_rate() -> RateLimit {
        RateLimit::new(5, 0.1)
    }

    pub const fn defaults() -> Self {
        Self {
            heartbeat: duration::seconds::<300>(),
            session_check: duration::seconds::<900>(),
            stream_shutdown_timeout: duration::seconds::<10>(),
            ping_enabled: false,
            max_connections: 10_000,
            connect_rate: Self::default_connect_rate(),
        }
    }
}

#[derive(ThisError, Debug)]
pub enum HubServiceBuilderError {
    #[error("Missing session service")]
    MissingSessionService,
    #[error("Missing hub registry")]
    MissingHubRegistry,
    #[error(transparent)]
    CommandInbox(#[from] CommandStreamError),
}

/// Construction-time resources shared by the hub and its stream egress.
pub struct HubServiceBuilder {
    config: HubConfig,
    connections: Arc<Connections>,
    control_sender: ControlQueue,
    control_receiver: mpsc::UnboundedReceiver<ControlMessage>,

    session_service: Option<Arc<CurrentUserService>>,
    hub_registry: Option<RedisHubConnectionDb>,
    stream_registry: Option<StreamRegistry<RedisCommandInboxDb, PgCommandLogDb>>,
    supervisor: TaskSupervisor,
}

impl HubServiceBuilder {
    fn new(config: HubConfig) -> Self {
        let (control_sender, control_receiver) = ControlQueue::new();
        Self {
            config,
            connections: Arc::new(Connections::new()),
            control_sender,
            control_receiver,
            session_service: None,
            hub_registry: None,
            stream_registry: None,
            supervisor: TaskSupervisor::new(),
        }
    }

    /// Reports the tasks of the hub that end unexpectedly to the given supervisor, which the service shares
    /// with its other components. Without it the hub watches its tasks on its own.
    #[must_use]
    pub fn with_task_supervisor(mut self, supervisor: TaskSupervisor) -> Self {
        self.supervisor = supervisor;
        self
    }

    #[must_use]
    pub fn with_session_service(mut self, session_service: Arc<CurrentUserService>) -> Self {
        self.session_service = Some(session_service);
        self
    }

    #[must_use]
    pub fn with_hub_registry(mut self, hub_registry: RedisHubConnectionDb) -> Self {
        self.hub_registry = Some(hub_registry);
        self
    }

    pub fn with_stream_registry<F>(mut self, stream_registry: F) -> Result<Self, CommandStreamError>
    where
        F: FnOnce(&HubStreamEgress) -> Result<StreamRegistry<RedisCommandInboxDb, PgCommandLogDb>, CommandStreamError>,
    {
        let stream_egress = HubStreamEgress::new(self.connections.clone(), self.control_sender.clone());
        self.stream_registry = Some(stream_registry(&stream_egress)?);
        Ok(self)
    }

    pub async fn build(self) -> Result<HubService, HubServiceBuilderError> {
        let Self {
            config,
            connections,
            control_sender,
            control_receiver,
            session_service,
            hub_registry,
            stream_registry,
            supervisor,
        } = self;

        let session_service = session_service.ok_or(HubServiceBuilderError::MissingSessionService)?;
        let hub_registry = hub_registry.ok_or(HubServiceBuilderError::MissingHubRegistry)?;
        let (shutdown_complete_tx, shutdown_complete_rx) = watch::channel(false);

        let service = HubService {
            inner: Arc::new(Inner {
                control_sender,
                shutdown_complete: shutdown_complete_rx,
                connections,
                subscribers: Subscribers::new(),
                hub_registry,
                stream_registry,
                ping_enabled: config.ping_enabled,
                admission: Admission::new(config.connect_rate, config.max_connections),
                supervisor,
            }),
        };

        // All consumer subscriptions must be registered before the dispatcher starts: the
        // consumers are event-sourced, so a lifecycle event dispatched before they subscribe is
        // lost to them forever. The start_* calls subscribe synchronously (awaited), and the
        // dispatcher is started last so nothing drains the channels until every subscription is in
        // place.
        let background_tasks = vec![
            HubService::start_registry_listener(service.clone()),
            Heartbeat::start(service.clone(), config.heartbeat).await,
            SessionChecker::start(service.clone(), session_service, config.session_check).await,
        ];
        let stream_dispatcher = match service.inner.stream_registry.clone() {
            Some(registry) => {
                registry.listen().await?;
                Some(StreamDispatcher::start(service.clone(), registry, config.stream_shutdown_timeout).await)
            }
            None => None,
        };
        HubService::start_dispatcher(
            service.clone(),
            control_receiver,
            background_tasks,
            stream_dispatcher,
            shutdown_complete_tx,
        );
        Ok(service)
    }
}

/// Counts a failed registry call by operation and passes the result on.
fn track_registry<T>(operation: &'static str, result: Result<T, HubError>) -> Result<T, HubError> {
    if result.is_err() {
        hub_metrics().registry_error(operation);
    }
    result
}

/// Registry rows removed at the same time on shutdown.
const SHUTDOWN_REGISTRY_CONCURRENCY: usize = 32;

/// Longest the registry cleanup of a shutdown takes. The rows left behind expire with their TTL.
const SHUTDOWN_REGISTRY_TIMEOUT: Duration = Duration::from_secs(5);

struct Inner {
    control_sender: ControlQueue,
    shutdown_complete: watch::Receiver<bool>,
    connections: Arc<Connections>,
    subscribers: Subscribers,
    hub_registry: RedisHubConnectionDb,
    stream_registry: Option<StreamRegistry<RedisCommandInboxDb, PgCommandLogDb>>,
    ping_enabled: bool,
    admission: Admission<Uuid>,
    supervisor: TaskSupervisor,
}

/// Messaging service for connected users.
#[derive(Clone)]
pub struct HubService {
    inner: Arc<Inner>,
}

impl HubService {
    pub fn builder(hub_config: HubConfig) -> HubServiceBuilder {
        HubServiceBuilder::new(hub_config)
    }

    pub fn ping_enabled(&self) -> bool {
        self.inner.ping_enabled
    }

    /// Watches the tasks that have to live as long as the hub.
    pub fn supervisor(&self) -> &TaskSupervisor {
        &self.inner.supervisor
    }

    /// Admits a connect of the user, before its socket is upgraded. The permit is held for as long as the
    /// socket is served.
    pub fn admit(&self, user_id: Uuid) -> Result<AdmissionPermit, AdmissionError> {
        self.inner.admission.admit(user_id)
    }

    /// Subscribes the connection to a stream (or asks for it to be synced again), as `requested_kind` if the
    /// stream is new. What the stream carries is then sent to the connection through the hub. This is the way
    /// for every transport.
    ///
    /// The stream methods of the hub trust their caller: the transport has authenticated the user, and holds
    /// the id the hub gave its connection. Neither is ever taken from the client. What the client does send
    /// (stream id, command) is checked here and in the registry.
    pub async fn subscribe_stream(
        &self,
        connection_id: Uuid,
        user_id: Uuid,
        stream_id: StreamId,
    ) -> Result<(), SubscribeFailure> {
        let registry = self.inner.stream_registry.as_ref().ok_or(SubscribeFailure::Failed)?;
        registry
            .open(connection_id, user_id, &stream_id)
            .await
            .map_err(|err| err.subscribe_failure())?;
        self.request_stream_subscribe(connection_id, stream_id)
            .map_err(|_| SubscribeFailure::Failed)
    }

    pub fn unsubscribe_stream(&self, connection_id: Uuid, stream_id: StreamId) -> Result<(), HubError> {
        if let Some(registry) = &self.inner.stream_registry {
            registry.close(connection_id, &stream_id);
        }
        self.request_stream_unsubscribe(connection_id, stream_id)
    }

    /// Queues a command of the connection, which has to be subscribed to the stream. It does not go through
    /// the hub's loop. The sender sees its command come back with its version, in the stream. A push into an
    /// inbox that stays full is refused as busy; the connection stays.
    pub async fn send_stream_command(
        &self,
        connection_id: Uuid,
        user_id: Uuid,
        stream_id: &StreamId,
        command_id: Uuid,
        command: &[u8],
    ) -> Result<(), CommandFailure> {
        let result = match &self.inner.stream_registry {
            Some(registry) => {
                registry
                    .push(connection_id, stream_id, user_id, command_id, command)
                    .await
            }
            None => Err(CommandFailure::NotSubscribed),
        };
        match result {
            Ok(()) => stream_metrics().command_queued(stream_id.kind()),
            Err(reason) => stream_metrics().command_refused(stream_id.kind(), reason),
        }
        result
    }

    fn send_control(&self, command: ControlCommand) -> Result<(), HubError> {
        self.inner.control_sender.send(command)
    }

    /// Logs and counts a connection removed by the hub, and tells its socket why. The reason is the one the
    /// socket was told, which differs from `reason` when something ended the connection first.
    fn record_drop(&self, removed: RemovedConnection, reason: DisconnectReason) -> (Uuid, Uuid, DisconnectReason) {
        let (user_id, connection_id) = (removed.user_id, removed.connection_id);
        let reason = removed.close(reason);
        tracing::info!("[{user_id}] [{connection_id}] Connection dropped: {reason}");
        hub_metrics().connection_dropped(reason);
        (user_id, connection_id, reason)
    }

    /// Prunes any connection found dead during an egress send.
    fn drop_dead(&self, dead: impl IntoIterator<Item = DeadConnection>) -> Result<(), HubError> {
        for DeadConnection { user_id, connection_id, reason } in dead {
            self.inner.control_sender.disconnect(user_id, connection_id, reason)?;
        }
        Ok(())
    }

    /// Requests a new connection for a user, superseding any prior one, and returns its id.
    pub fn request_connection(
        &self,
        user_id: Uuid,
        session_key: SessionKey,
        socket_sender: SocketSender,
        topics: Vec<TopicKey>,
    ) -> Result<Uuid, HubError> {
        let connection_id = Uuid::new_v4();
        self.send_control(ControlCommand::ConnectUser {
            user_id,
            connection_id,
            session_key,
            socket_sender,
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
        self.send_control(ControlCommand::DisconnectUser { user_id, connection_id, reason })
    }

    /// Subscribes the connection to a stream, or asks for it to be synced again if it already is.
    /// Ignored by the loop when the connection is no longer registered.
    pub fn request_stream_subscribe(&self, connection_id: Uuid, stream_id: StreamId) -> Result<(), HubError> {
        self.send_control(ControlCommand::SubscribeStream { connection_id, stream_id })
    }

    pub fn request_stream_unsubscribe(&self, connection_id: Uuid, stream_id: StreamId) -> Result<(), HubError> {
        self.send_control(ControlCommand::UnsubscribeStream { connection_id, stream_id })
    }

    /// Requests hub shutdown.
    pub fn request_shutdown(&self) -> Result<(), HubError> {
        self.send_control(ControlCommand::Shutdown)
    }

    pub async fn wait_shutdown_complete(&self) {
        let mut shutdown_complete = self.inner.shutdown_complete.clone();
        while !*shutdown_complete.borrow_and_update() {
            if shutdown_complete.changed().await.is_err() {
                return;
            }
        }
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
    #[tracing::instrument(level = "debug", skip_all, fields(enduser.id = %user_id))]
    pub async fn send_to_user(&self, user_id: Uuid, message: HubMessage) -> Result<(), HubError> {
        self.drop_dead(self.inner.connections.send_to_user(user_id, message).await.err())
    }

    /// Broadcasts an egress message to every connection subscribed to its topic.
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

    /// Live connection and subscriber counts for status reporting.
    pub async fn stats(&self) -> HubStats {
        let connections = self.inner.connections.len().await;
        let subscribers = self.inner.subscribers.len().await;
        HubStats { connections, subscribers }
    }

    /// Starts the command loop, the sole writer of connection state.
    fn start_dispatcher(
        service: HubService,
        mut control_receiver: mpsc::UnboundedReceiver<ControlMessage>,
        background_tasks: Vec<JoinHandle<()>>,
        stream_dispatcher: Option<JoinHandle<()>>,
        shutdown_complete: watch::Sender<bool>,
    ) {
        let supervisor = service.supervisor().clone();
        supervisor.spawn("hub command loop", async move {
            while let Some((command, origin, queued_at)) = control_receiver.recv().await {
                let name = command.name();
                let metrics = hub_metrics();
                metrics.control_command_started(name, queued_at.elapsed());

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
                let keep_running = service.process(command).instrument(span).await;
                metrics.control_command_finished(name, start.elapsed());
                if !keep_running {
                    break;
                }
            }
            tracing::info!("Hub command loop stopped");
            for task in &background_tasks {
                task.abort();
            }
            for task in background_tasks {
                if let Err(err) = task.await {
                    if !err.is_cancelled() {
                        tracing::error!("Hub background task failed while stopping: {err}");
                    }
                }
            }
            // Not aborted: on the Shutdown event it has the stream workers make their last commands durable,
            // bounded by `stream_shutdown_timeout`, and the process must not end before that.
            if let Some(dispatcher) = stream_dispatcher {
                if let Err(err) = dispatcher.await {
                    tracing::error!("Stream dispatcher failed while stopping: {err}");
                }
            }
            drop(control_receiver);
            shutdown_complete.send_replace(true);
        });
    }

    fn start_registry_listener(service: HubService) -> JoinHandle<()> {
        tokio::spawn(async move {
            let notifier = service.clone();
            if let Err(err) = service
                .inner
                .hub_registry
                .listen_to_registry_changes(move |user_id| {
                    if let Err(err) = notifier.notify_registry_changed(user_id) {
                        tracing::warn!("[{user_id}] Failed to enqueue hub registry change command: {err:#?}");
                    }
                })
                .await
            {
                tracing::error!("Failed to listen to hub registry changes: {err:#?}");
                service.supervisor().task_ended("hub registry listener");
            }
        })
    }

    /// Processes one control command. Returns if the dispatcher should continue (true) or exit (false).
    async fn process(&self, command: ControlCommand) -> bool {
        tracing::debug!("Processing command: {command:#?}");
        match command {
            ControlCommand::ConnectUser {
                user_id,
                connection_id,
                session_key,
                socket_sender,
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
                            .unregister_connection(user_id, Some(old_connection_id))
                            .await
                        {
                            let (user_id, connection_id, reason) =
                                self.record_drop(removed, DisconnectReason::Replaced);
                            self.publish(HubEvent::UserDisconnected { user_id, connection_id, reason })
                                .await;
                        }
                    }
                }

                self.inner
                    .connections
                    .register_connection(user_id, connection_id, session_key, socket_sender, topics)
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
                    .unregister_connection(user_id, Some(connection_id))
                    .await
                else {
                    return true;
                };

                // A client-requested disconnect is not a drop; its socket is already closing.
                let (removed_connection_id, reason) = if reason.is_drop() {
                    let (_, connection_id, reason) = self.record_drop(removed, reason);
                    (connection_id, reason)
                } else {
                    (removed.connection_id, removed.close(reason))
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
            ControlCommand::HubRegistryChanged { user_id } => {
                self.process_registry_change(user_id).await;
                true
            }
            ControlCommand::SubscribeStream { connection_id, stream_id } => {
                // A subscribe that comes after the connection is gone must not start anything.
                if self.inner.connections.contains(connection_id).await {
                    self.publish_internal(StreamEvent::Subscribed { connection_id, stream_id })
                        .await;
                } else if let Some(registry) = &self.inner.stream_registry {
                    // It was opened after the connection was released: close it again.
                    registry.disconnect(connection_id);
                }
                true
            }
            ControlCommand::UnsubscribeStream { connection_id, stream_id } => {
                if self.inner.connections.contains(connection_id).await {
                    self.publish_internal(StreamEvent::Unsubscribed { connection_id, stream_id })
                        .await;
                }
                true
            }
            ControlCommand::StreamFailed { stream_id } => {
                // The registry is closed first: a client that subscribes again on the failure finds it clean.
                let connections = self
                    .inner
                    .stream_registry
                    .as_ref()
                    .map(|registry| registry.close_stream(&stream_id))
                    .unwrap_or_default();
                for connection_id in connections {
                    self.publish_internal(StreamEvent::Unsubscribed {
                        connection_id,
                        stream_id: stream_id.clone(),
                    })
                    .await;
                    let message = StreamMessage::SubscribeFailed {
                        stream_id: stream_id.clone(),
                        reason: SubscribeFailure::Failed,
                    };
                    StreamEgress::send(self, connection_id, Arc::new(message)).await;
                }
                true
            }
            ControlCommand::Shutdown => {
                self.inner.supervisor.shutdown_started();
                self.shutdown().await;
                false
            }
        }
    }

    async fn create_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        let result = async {
            let mut context = self.inner.hub_registry.create_context().await?;
            context.create_connection(user_id, connection_id).await
        }
        .await;
        track_registry("create", result)
    }

    /// Whether the registry holds `connection_id` as the user's active connection; `false` on a read error.
    async fn is_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> bool {
        let found = match self.inner.hub_registry.create_context().await {
            Ok(mut context) => context.find_connection_by_user(user_id).await,
            Err(err) => Err(err),
        };
        match track_registry("find", found) {
            Ok(found) => found.is_some_and(|found| found.connection_id == connection_id),
            Err(err) => {
                tracing::warn!("[{user_id}] Failed to read hub connection: {err:#?}");
                false
            }
        }
    }

    async fn remove_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        let result = async {
            let mut context = self.inner.hub_registry.create_context().await?;
            context.remove_connection_if_active(user_id, connection_id).await
        }
        .await;
        track_registry("remove", result).map(|_| ())
    }

    pub async fn heartbeat_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<bool, HubError> {
        let result = async {
            let mut context = self.inner.hub_registry.create_context().await?;
            context.heartbeat_connection(user_id, connection_id).await
        }
        .await;
        track_registry("heartbeat", result)
    }

    /// Batched heartbeat over the given connections, returning the ones the registry no longer holds
    /// as active. See [`HubRegistry::heartbeat_connections`] for the reconciliation semantics.
    pub async fn heartbeat_registry_connections(
        &self,
        connections: &[HubConnection],
    ) -> Result<Vec<HubConnection>, HubError> {
        let result = async {
            let mut context = self.inner.hub_registry.create_context().await?;
            context.heartbeat_connections(connections).await
        }
        .await;
        track_registry("heartbeat", result)
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

        if is_active {
            return;
        }

        // Force removal of the connection we just found to be inactive in the registry. If a
        // fresh reconnect replaced it in the meantime, the connection ids differ and we leave
        // the new connection untouched.
        let Some(removed) = self
            .inner
            .connections
            .unregister_connection(user_id, Some(connection_id))
            .await
        else {
            return;
        };

        let (user_id, connection_id, reason) = self.record_drop(removed, DisconnectReason::RegistryInactive);
        self.publish(HubEvent::UserDisconnected { user_id, connection_id, reason })
            .await;
    }

    /// Stops the internal consumers, then drops every connection. Nothing is published after the
    /// Shutdown event: the consumers are gone, and the sockets are told directly.
    async fn shutdown(&self) {
        self.inner
            .subscribers
            .publish(HubMessage::Hub(HubEvent::Shutdown))
            .await;
        self.inner.subscribers.clear().await;

        // The sockets are told first, whatever state the registry is in.
        let dropped: Vec<(Uuid, Uuid)> = self
            .inner
            .connections
            .unregister_all()
            .await
            .into_iter()
            .map(|removed| {
                let (user_id, connection_id, _) = self.record_drop(removed, DisconnectReason::Shutdown);
                (user_id, connection_id)
            })
            .collect();

        // Side by side and within a time limit: a slow registry would otherwise cost every connection its
        // own wait one after the other, and the process may be killed before it is done.
        let cleanup = async {
            if let Err(err) = self.inner.hub_registry.unlisten_registry_changes().await {
                tracing::warn!("Failed to stop listening to hub registry changes: {err:#?}");
            }
            stream::iter(dropped)
                .for_each_concurrent(SHUTDOWN_REGISTRY_CONCURRENCY, |(user_id, connection_id)| async move {
                    if let Err(err) = self.remove_registry_connection(user_id, connection_id).await {
                        tracing::warn!(
                            "[{user_id}] Failed to remove hub connection on shutdown (TTL will clean up): {err:#?}"
                        );
                    }
                })
                .await;
        };
        if tokio::time::timeout(SHUTDOWN_REGISTRY_TIMEOUT, cleanup).await.is_err() {
            tracing::warn!(
                "The registry cleanup did not finish within {SHUTDOWN_REGISTRY_TIMEOUT:?}, TTL will clean up"
            );
        }
    }

    /// Delivers a stream event to the internal subscribers only: subscribing and unsubscribing must never
    /// load the sockets' queues.
    async fn publish_internal(&self, event: StreamEvent) {
        self.inner.subscribers.publish(HubMessage::StreamEvent(event)).await;
    }

    /// Delivers a lifecycle `HubEvent` to every internal subscriber and every topic-matching
    /// connection, pruning any connection found dead.
    async fn publish(&self, event: HubEvent) {
        let mut pending = VecDeque::from([event]);
        while let Some(event) = pending.pop_front() {
            if let HubEvent::UserDisconnected { connection_id, .. } = &event {
                if let Some(registry) = &self.inner.stream_registry {
                    registry.disconnect(*connection_id);
                }
            }
            let message = HubMessage::Hub(event);
            // Internal subscribers first: only the loop delivers to them.
            self.inner.subscribers.publish(message.clone()).await;

            if let Err(dead) = self.inner.connections.broadcast(message).await {
                for DeadConnection { user_id, connection_id, reason } in dead {
                    if let Some(removed) = self
                        .inner
                        .connections
                        .unregister_connection(user_id, Some(connection_id))
                        .await
                    {
                        let (user_id, connection_id, reason) = self.record_drop(removed, reason);
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

impl StreamEgress for HubService {
    async fn send(&self, connection_id: Uuid, message: Arc<StreamMessage>) -> bool {
        match self
            .send_to_connection(connection_id, HubMessage::StreamMessage(message))
            .await
        {
            Ok(queued) => queued,
            Err(err) => {
                tracing::warn!("[{connection_id}] Failed to send a stream message: {err:#?}");
                false
            }
        }
    }

    async fn notify_failed(&self, stream_id: &StreamId) {
        if let Err(err) = self.send_control(ControlCommand::StreamFailed { stream_id: stream_id.clone() }) {
            tracing::warn!("[{stream_id}] Failed to queue the stream failure: {err:#?}");
        }
    }
}
