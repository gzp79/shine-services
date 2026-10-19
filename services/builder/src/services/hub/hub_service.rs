use crate::services::hub::hub_metrics::hub_metrics;
use crate::{
    models::{
        messages::{DisconnectReason, HubEvent, HubMessage, StreamEvent, StreamResume, TopicKey},
        HubError,
    },
    repositories::hub_registry::{redis::RedisHubConnectionDb, HubConnection, HubConnectionDb, HubRegistry},
    services::{
        hub::{
            hub_command::ControlCommand,
            hub_connections::{Connections, DeadConnection, RemovedConnection},
            hub_subscribers::Subscribers,
            tasks::{ChatDispatcher, Heartbeat, SessionChecker, StreamDispatcher},
        },
        ChatService, CommandStreamService,
    },
};
use opentelemetry::trace::SpanContext;
use shine_infra::db::command_log::StreamId;
use shine_infra::session::{CurrentUserService, SessionKey};
use shine_infra::telemetry::{current_span_context, label, link_span};
use std::collections::VecDeque;
use std::time::Instant;
use std::{sync::Arc, time::Duration};
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::{info_span, Instrument};
use uuid::Uuid;

/// Cadence configuration for the hub's periodic connection consumers.
pub struct HubIntervals {
    pub heartbeat: Duration,
    pub session_check: Duration,
    pub chat: Duration,
}

/// Point-in-time counts of hub state, for status reporting.
#[derive(Clone, Debug)]
pub struct HubStats {
    pub connections: usize,
    pub subscribers: usize,
}

struct Inner {
    control_tx: mpsc::UnboundedSender<(ControlCommand, SpanContext, Instant)>,
    connections: Connections,
    subscribers: Subscribers,
    hub_registry: RedisHubConnectionDb,
}

/// Messaging service for connected users.
#[derive(Clone)]
pub struct HubService {
    inner: Arc<Inner>,
}

impl HubService {
    pub async fn new(
        hub_registry: RedisHubConnectionDb,
        session_service: Arc<CurrentUserService>,
        chat_service: ChatService,
        intervals: HubIntervals,
        command_streams: Option<CommandStreamService>,
    ) -> Self {
        let (control_tx, control_rx) = mpsc::unbounded_channel();

        let service = Self {
            inner: Arc::new(Inner {
                control_tx,
                connections: Connections::new(),
                subscribers: Subscribers::new(),
                hub_registry,
            }),
        };

        // All consumer subscriptions must be registered before the dispatcher starts: the
        // consumers are event-sourced, so a lifecycle event dispatched before they subscribe is
        // lost to them forever. The start_* calls subscribe synchronously (awaited), and the
        // dispatcher is started last so nothing drains the channels until every subscription is in
        // place.
        let mut background_tasks = vec![
            Self::start_registry_listener(service.clone()),
            Heartbeat::start(service.clone(), intervals.heartbeat).await,
            SessionChecker::start(service.clone(), session_service, intervals.session_check).await,
            ChatDispatcher::start(service.clone(), chat_service, intervals.chat).await,
        ];
        if let Some(command_streams) = command_streams {
            background_tasks.push(StreamDispatcher::start(service.clone(), command_streams).await);
        }
        Self::start_dispatcher(service.clone(), control_rx, background_tasks);
        service
    }

    fn send_control(&self, command: ControlCommand) -> Result<(), HubError> {
        self.inner
            .control_tx
            .send((command, current_span_context(), Instant::now()))
            .map_err(|_| HubError::SendCommandFailed)
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
            self.send_control(ControlCommand::DropConnection { connection_id, reason })?;
        }
        Ok(())
    }

    /// Requests a new connection for a user, superseding any prior one, and returns its id.
    pub fn request_connection(
        &self,
        user_id: Uuid,
        session_key: SessionKey,
        tx: mpsc::Sender<HubMessage>,
        topics: Vec<TopicKey>,
    ) -> Result<Uuid, HubError> {
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
        self.send_control(ControlCommand::DisconnectUser { user_id, connection_id, reason })
    }

    /// Requests hub shutdown.
    pub fn request_shutdown(&self) -> Result<(), HubError> {
        self.send_control(ControlCommand::Shutdown)
    }

    /// Notifies the hub that a user's registry entry changed, prompting an active-connection recheck.
    pub fn notify_registry_changed(&self, user_id: Uuid) -> Result<(), HubError> {
        self.send_control(ControlCommand::HubRegistryChanged { user_id })
    }

    /// Requests a stream subscription for a connection; ignored if the connection is gone by then.
    #[allow(dead_code)] // called by the socket routes (S7)
    pub fn request_subscribe_stream(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
        stream_id: StreamId,
        kind: &'static str,
        resume: Option<StreamResume>,
    ) -> Result<(), HubError> {
        self.send_control(ControlCommand::SubscribeStream {
            user_id,
            connection_id,
            stream_id,
            kind,
            resume,
        })
    }

    /// Ends a stream subscription of a connection; the connection itself stays.
    #[allow(dead_code)] // called by the socket routes (S7)
    pub fn request_unsubscribe_stream(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
        stream_id: StreamId,
    ) -> Result<(), HubError> {
        self.send_control(ControlCommand::UnsubscribeStream {
            user_id,
            connection_id,
            stream_id,
        })
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

    /// Live connection and subscriber counts for status reporting.
    pub async fn stats(&self) -> HubStats {
        let connections = self.inner.connections.len().await;
        let subscribers = self.inner.subscribers.len().await;
        HubStats { connections, subscribers }
    }

    /// Starts the command loop, the sole writer of connection state.
    fn start_dispatcher(
        service: HubService,
        mut control_rx: mpsc::UnboundedReceiver<(ControlCommand, SpanContext, Instant)>,
        background_tasks: Vec<JoinHandle<()>>,
    ) {
        tokio::spawn(async move {
            while let Some((command, origin, queued_at)) = control_rx.recv().await {
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
                let keep_running = service.process(command).instrument(span).await;
                metrics.commands.add(1, &[label("command", name)]);
                metrics
                    .command_duration
                    .record(start.elapsed().as_secs_f64(), &[label("command", name)]);
                if !keep_running {
                    break;
                }
            }
            tracing::info!("Hub command loop stopped");
            for task in background_tasks {
                task.abort();
            }
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
            ControlCommand::SubscribeStream {
                user_id,
                connection_id,
                stream_id,
                kind,
                resume,
            } => {
                // A subscribe racing a replaced or dropped connection must not pin a stream worker.
                if self.inner.connections.contains(connection_id).await {
                    self.publish_stream_event(StreamEvent::Subscribed {
                        connection_id,
                        stream_id,
                        kind,
                        resume,
                    })
                    .await;
                } else {
                    tracing::debug!("[{user_id}] [{connection_id}] Stream subscribe for an unknown connection ignored");
                }
                true
            }
            ControlCommand::UnsubscribeStream { connection_id, stream_id, .. } => {
                if self.inner.connections.contains(connection_id).await {
                    self.publish_stream_event(StreamEvent::Unsubscribed { connection_id, stream_id })
                        .await;
                }
                true
            }
            ControlCommand::Shutdown => {
                self.shutdown().await;
                false
            }
        }
    }

    async fn create_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        let mut context = self.inner.hub_registry.create_context().await?;
        context.create_connection(user_id, connection_id).await
    }

    /// Whether the registry holds `connection_id` as the user's active connection; `false` on a read error.
    async fn is_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> bool {
        let found = match self.inner.hub_registry.create_context().await {
            Ok(mut context) => context.find_connection_by_user(user_id).await,
            Err(err) => Err(err),
        };
        match found {
            Ok(found) => found.is_some_and(|found| found.connection_id == connection_id),
            Err(err) => {
                tracing::warn!("[{user_id}] Failed to read hub connection: {err:#?}");
                false
            }
        }
    }

    async fn remove_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        let mut context = self.inner.hub_registry.create_context().await?;
        context.remove_connection_if_active(user_id, connection_id).await?;
        Ok(())
    }

    pub(crate) async fn heartbeat_registry_connection(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> Result<bool, HubError> {
        let mut context = self.inner.hub_registry.create_context().await?;
        context.heartbeat_connection(user_id, connection_id).await
    }

    /// Batched heartbeat over the given connections, returning the ones the registry no longer holds
    /// as active. See [`HubRegistry::heartbeat_connections`] for the reconciliation semantics.
    pub(crate) async fn heartbeat_registry_connections(
        &self,
        connections: &[HubConnection],
    ) -> Result<Vec<HubConnection>, HubError> {
        let mut context = self.inner.hub_registry.create_context().await?;
        context.heartbeat_connections(connections).await
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

    /// Stops the internal consumers, then drops every connection. Nothing is published after the
    /// Shutdown event: the consumers are gone, and the sockets are told directly.
    async fn shutdown(&self) {
        self.inner
            .subscribers
            .publish(HubMessage::Hub(HubEvent::Shutdown))
            .await;
        self.inner.subscribers.clear().await;

        if let Err(err) = self.inner.hub_registry.unlisten_registry_changes().await {
            tracing::warn!("Failed to stop listening to hub registry changes: {err:#?}");
        }

        for removed in self.inner.connections.remove_all().await {
            let (user_id, connection_id) = self.record_drop(removed, DisconnectReason::Shutdown);
            if let Err(err) = self.remove_registry_connection(user_id, connection_id).await {
                tracing::warn!("[{user_id}] Failed to remove hub connection on shutdown (TTL will clean up): {err:#?}");
            }
        }
    }

    /// Delivers a stream subscription change to the internal subscribers; no socket subscribes to its topic.
    /// A disconnect needs no such event: the stream dispatcher drops the streams of a connection when it
    /// sees `UserDisconnected`.
    async fn publish_stream_event(&self, event: StreamEvent) {
        self.inner.subscribers.publish(HubMessage::StreamEvent(event)).await;
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
