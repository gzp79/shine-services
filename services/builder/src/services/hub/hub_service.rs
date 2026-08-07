use crate::{
    models::{
        messages::{HubEvent, HubMessage, TopicKey},
        HubError,
    },
    repositories::hub_registry::{redis::RedisHubConnectionDb, HubConnection, HubConnectionDb, HubRegistry},
    services::{
        hub::{
            hub_command::ControlCommand,
            hub_connections::Connections,
            hub_subscribers::Subscribers,
            tasks::{ChatDispatcher, Heartbeat, SessionChecker},
        },
        ChatService,
    },
};
use shine_infra::session::{CurrentUserService, SessionKey};
use std::collections::VecDeque;
use std::{sync::Arc, time::Duration};
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

/// Cadence configuration for the hub's periodic connection consumers.
pub struct HubIntervals {
    pub heartbeat: Duration,
    pub session_check: Duration,
    pub chat: Duration,
}

/// Point-in-time counts of hub state, for status reporting.
#[derive(Clone, Copy, Debug)]
pub struct HubStats {
    pub connections: usize,
    pub subscribers: usize,
}

struct Inner {
    control_tx: mpsc::UnboundedSender<ControlCommand>,
    connections: Connections,
    subscribers: Subscribers,
    hub_registry: RedisHubConnectionDb,
}

/// Messaging service for the connected users.
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
        let background_tasks = vec![
            Self::start_registry_listener(service.clone()),
            Heartbeat::start(service.clone(), intervals.heartbeat).await,
            SessionChecker::start(service.clone(), session_service, intervals.session_check).await,
            ChatDispatcher::start(service.clone(), chat_service, intervals.chat).await,
        ];
        Self::start_dispatcher(service.clone(), control_rx, background_tasks);
        service
    }

    fn send_control(&self, command: ControlCommand) -> Result<(), HubError> {
        self.inner
            .control_tx
            .send(command)
            .map_err(|_| HubError::SendCommandFailed)
    }

    /// Prunes any connection found dead during an egress send.
    fn drop_dead(&self, dead: impl IntoIterator<Item = Uuid>) -> Result<(), HubError> {
        for connection_id in dead {
            self.send_control(ControlCommand::DropConnection { connection_id })?;
        }
        Ok(())
    }

    /// Requests a new connection for a user, returning its connection id. Supersedes any prior
    /// connection for that user.
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

    /// Requests removal of a connection, but only if `connection_id` still matches the user's active
    /// one, so a stale request cannot tear down a fresh reconnect.
    pub fn request_disconnection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        self.send_control(ControlCommand::DisconnectUser { user_id, connection_id })
    }

    /// Requests hub shutdown.
    pub fn request_shutdown(&self) -> Result<(), HubError> {
        self.send_control(ControlCommand::Shutdown)
    }

    /// Notifies the hub that a user's registry entry changed, prompting an active-connection recheck.
    pub fn notify_registry_changed(&self, user_id: Uuid) -> Result<(), HubError> {
        self.send_control(ControlCommand::HubRegistryChanged { user_id })
    }

    /// Sends an egress message to a specific connection.
    pub async fn send_to_connection(&self, connection_id: Uuid, message: HubMessage) -> Result<(), HubError> {
        self.drop_dead(
            self.inner
                .connections
                .send_to_connection(connection_id, message)
                .await
                .err(),
        )
    }

    /// Sends an egress message to a user's active connection.
    pub async fn send_to_user(&self, user_id: Uuid, message: HubMessage) -> Result<(), HubError> {
        self.drop_dead(self.inner.connections.send_to_user(user_id, message).await.err())
    }

    /// Broadcasts an egress message to every connection subscribed to its topic.
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

    /// Subscribe to a set of topics on an unbounded, lossless channel. For internal consumers
    /// only — a dropped lifecycle event would corrupt a consumer's connection tracker. Addressable
    /// connections do not subscribe; they hand the hub their own bounded egress channel at connect
    /// time.
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
        mut control_rx: mpsc::UnboundedReceiver<ControlCommand>,
        background_tasks: Vec<JoinHandle<()>>,
    ) {
        tokio::spawn(async move {
            while let Some(command) = control_rx.recv().await {
                if !service.process(command).await {
                    break;
                }
            }
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
                        log::error!("[{user_id}] Failed to enqueue hub registry change command: {err:#?}");
                    }
                })
                .await
            {
                log::error!("Failed to listen to hub registry changes: {err:#?}");
            }
        })
    }

    /// Processes one control command. Returns if the dispatcher should continue (true) or exit (false).
    async fn process(&self, command: ControlCommand) -> bool {
        log::debug!("Processing command: {command:#?}");
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
                    log::error!("[{user_id}] Failed to create hub connection: {err:#?}");
                    return true;
                }

                // Now that the new connection is durably registered, tear down any different
                // connection that was active for this user, so the old socket closes and the
                // connection tracker drops it. Same-id re-registration is not a replacement.
                if let Some((old_connection_id, _)) = self.inner.connections.find_active(user_id).await {
                    if old_connection_id != connection_id {
                        self.inner
                            .connections
                            .remove_connection(user_id, Some(old_connection_id))
                            .await;
                        self.publish(HubMessage::Hub(HubEvent::UserDisconnected {
                            user_id,
                            connection_id: old_connection_id,
                        }))
                        .await;
                    }
                }

                self.inner
                    .connections
                    .register_connection(user_id, connection_id, session_key, tx, topics)
                    .await;
                self.publish(HubMessage::Hub(HubEvent::UserConnected {
                    user_id,
                    connection_id,
                    session_key,
                }))
                .await;
                true
            }
            ControlCommand::DisconnectUser { user_id, connection_id } => {
                // Only removes the entry when `connection_id` still matches the active
                // connection, so a stale disconnect can never tear down a fresh reconnect.
                let Some(removed_connection_id) = self
                    .inner
                    .connections
                    .remove_connection(user_id, Some(connection_id))
                    .await
                else {
                    return true;
                };

                if let Err(err) = self.remove_registry_connection(user_id, removed_connection_id).await {
                    // Best-effort: publish UserDisconnected regardless so the socket and trackers
                    // never wedge, and let the registry TTL clean up the stale row.
                    log::error!("[{user_id}] Failed to remove hub connection (relying on TTL cleanup): {err:#?}");
                }

                self.publish(HubMessage::Hub(HubEvent::UserDisconnected {
                    user_id,
                    connection_id: removed_connection_id,
                }))
                .await;
                true
            }
            ControlCommand::DropConnection { connection_id } => {
                // A producer found this connection's channel dead during an off-loop send. Prune it
                // here so all mutation stays on the loop. Force-remove by id (a fresh reconnect that
                // replaced it has a different id and its channel would not be dead).
                if let Some((user_id, _)) = self.inner.connections.remove_connection_by_id(connection_id).await {
                    if let Err(err) = self.remove_registry_connection(user_id, connection_id).await {
                        log::error!("[{user_id}] Failed to remove dropped connection from registry (TTL will clean up): {err:#?}");
                    }
                    self.publish(HubMessage::Hub(HubEvent::UserDisconnected { user_id, connection_id }))
                        .await;
                }
                true
            }
            ControlCommand::HubRegistryChanged { user_id } => {
                self.process_registry_change(user_id).await;
                true
            }
            ControlCommand::Shutdown => {
                self.publish(HubMessage::Hub(HubEvent::Shutdown)).await;
                false
            }
        }
    }

    async fn create_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        let mut context = self.inner.hub_registry.create_context().await?;
        context.create_connection(user_id, connection_id).await
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

    /// Batched heartbeat over all locally-tracked connections on a single pooled connection.
    /// Returns the connections the registry no longer holds as active, which the caller should
    /// disconnect. See [`HubRegistry::heartbeat_connections`] for the reconciliation semantics.
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
                log::error!("[{user_id}] Failed to heartbeat hub connection: {err:#?}");
                return;
            }
        };

        if is_active {
            return;
        }

        // Force removal of the connection we just found to be inactive in the registry. If a
        // fresh reconnect replaced it in the meantime, the connection ids differ and we leave
        // the new connection untouched.
        let Some(removed_connection_id) = self
            .inner
            .connections
            .remove_connection(user_id, Some(connection_id))
            .await
        else {
            return;
        };

        self.publish(HubMessage::Hub(HubEvent::UserDisconnected {
            user_id,
            connection_id: removed_connection_id,
        }))
        .await;
    }

    /// Loop-side broadcast: fans a message out to every topic-matching connection *and* every
    /// internal subscriber (heartbeat, session checker), pruning any connection whose channel is
    /// found dead. Because it touches the subscriber list, this runs only on the command loop, so
    /// subscriber mutation stays single-writer. Pruning emits a `UserDisconnected`, which is itself
    /// broadcast — handled iteratively via a work queue rather than recursion so this stays a plain
    /// `async fn` (async recursion would require boxing).
    async fn publish(&self, message: HubMessage) {
        let mut pending = VecDeque::from([message]);
        while let Some(message) = pending.pop_front() {
            // Internal subscribers first: only the loop delivers to them.
            self.inner.subscribers.publish(message.clone()).await;

            if let Err(dead) = self.inner.connections.broadcast(message).await {
                for connection_id in dead {
                    if let Some((user_id, _)) = self.inner.connections.remove_connection_by_id(connection_id).await {
                        if let Err(err) = self.remove_registry_connection(user_id, connection_id).await {
                            log::error!(
                                "[{user_id}] Failed to remove dead connection from registry (TTL will clean up): {err:#?}"
                            );
                        }
                        pending.push_back(HubMessage::Hub(HubEvent::UserDisconnected { user_id, connection_id }));
                    }
                }
            }
        }
    }
}
