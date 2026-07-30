use crate::{
    models::messages::{HubEvent, HubMessage, TopicKey, Workload},
    repositories::hub_registry::{
        redis::RedisHubConnectionDb, HubConnection, HubConnectionDb, HubConnectionError, HubRegistry,
    },
    services::{
        hub::{
            connected_users::ConnectedUsers,
            heartbeat::HeartbeatTask,
            hub_command::ControlCommand,
            hub_connection::{HubReceiver, HubSender},
        },
        run_connection_loop,
        session_checker::SessionChecker,
    },
};
use shine_infra::session::CurrentUserService;
use std::{sync::Arc, time::Duration};
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

/// Cadence configuration for the hub's periodic connection consumers.
pub struct HubIntervals {
    pub heartbeat: Duration,
    pub session_check: Duration,
}

/// Point-in-time counts of hub state, for status reporting.
#[derive(Clone, Copy, Debug)]
pub struct HubStats {
    pub connections: usize,
    pub subscribers: usize,
}

struct Inner {
    control_tx: mpsc::UnboundedSender<ControlCommand>,
    payload_tx: mpsc::Sender<Workload>,
    users: ConnectedUsers,
    hub_registry: RedisHubConnectionDb,
}

/// Messaging service for connected users and processes.
/// Commands are submitted through a HubSender; subscribers receive events
/// through a topic-filtered BusSubscription.
#[derive(Clone)]
pub struct HubService {
    inner: Arc<Inner>,
}

impl HubService {
    pub async fn new(
        hub_registry: RedisHubConnectionDb,
        session_service: Arc<CurrentUserService>,
        intervals: HubIntervals,
    ) -> Self {
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let (payload_tx, payload_rx) = mpsc::channel(128);

        let service = Self {
            inner: Arc::new(Inner {
                control_tx,
                payload_tx,
                users: ConnectedUsers::new(),
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
            Self::start_heartbeat(service.clone(), intervals.heartbeat).await,
            Self::start_session_checker(service.clone(), session_service, intervals.session_check).await,
        ];
        Self::start_dispatcher(service.clone(), control_rx, payload_rx, background_tasks);
        service
    }

    pub fn sender(&self) -> HubSender {
        HubSender::new(self.inner.control_tx.clone(), self.inner.payload_tx.clone())
    }

    /// Subscribe to a set of topics on an unbounded, lossless channel. For internal consumers
    /// only — a dropped lifecycle event would corrupt a consumer's connection tracker.
    pub async fn subscribe(&self, topics: Vec<TopicKey>) -> HubReceiver {
        let (tx, rx) = mpsc::unbounded_channel();
        self.inner.users.subscribe(topics, tx).await;
        HubReceiver::new(rx)
    }

    /// Subscribe to a set of topics on a bounded channel of the given `capacity`. A subscriber too
    /// slow to drain the broadcast has its subscription dropped once the buffer fills, which closes
    /// the receiver, rather than letting the hub buffer without limit. Callers choose a capacity
    /// appropriate to their consumer; the hub itself is transport-agnostic.
    pub async fn subscribe_bounded(&self, topics: Vec<TopicKey>, capacity: usize) -> HubReceiver {
        let (tx, rx) = mpsc::channel(capacity);
        self.inner.users.subscribe_bounded(topics, tx).await;
        HubReceiver::new_bounded(rx)
    }

    /// Live connection and subscriber counts for status reporting.
    pub async fn stats(&self) -> HubStats {
        let (connections, subscribers) = self.inner.users.stats().await;
        HubStats { connections, subscribers }
    }

    /// Starts the periodic registry heartbeat on its own connection loop, refreshing TTLs for
    /// locally-tracked connections and disconnecting any the registry no longer holds as active.
    async fn start_heartbeat(service: HubService, interval: Duration) -> JoinHandle<()> {
        let subscription = service.subscribe(vec![TopicKey::Hub]).await;
        tokio::spawn(async move {
            let task = HeartbeatTask::new(service);
            run_connection_loop(subscription, interval, task).await;
        })
    }

    /// Starts the periodic session checker on its own connection loop, validating each tracked
    /// session and issuing a targeted disconnect on expiry.
    async fn start_session_checker(
        service: HubService,
        session_service: Arc<CurrentUserService>,
        interval: Duration,
    ) -> JoinHandle<()> {
        let subscription = service.subscribe(vec![TopicKey::Hub]).await;
        tokio::spawn(async move {
            let checker = SessionChecker::new(session_service, &service);
            run_connection_loop(subscription, interval, checker).await;
        })
    }

    /// Starts the central dispatch loop that drains the control and payload channels, biased so
    /// lifecycle signals are handled ahead of broadcast payloads.
    fn start_dispatcher(
        service: HubService,
        mut control_rx: mpsc::UnboundedReceiver<ControlCommand>,
        mut payload_rx: mpsc::Receiver<Workload>,
        background_tasks: Vec<JoinHandle<()>>,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Control is prioritized so lifecycle signals are handled promptly.
                    biased;
                    control = control_rx.recv() => match control {
                        Some(command) => if !service.process(command).await { break },
                        None => break,
                    },
                    payload = payload_rx.recv() => match payload {
                        // Broadcast verbatim; conversion is a pure move with no hub processing.
                        Some(workload) => service.publish(workload.into()).await,
                        None => break,
                    },
                }
            }
            for task in background_tasks {
                task.abort();
            }
        });
    }

    fn start_registry_listener(service: HubService) -> JoinHandle<()> {
        tokio::spawn(async move {
            let sender = service.sender();
            if let Err(err) = service
                .inner
                .hub_registry
                .listen_to_registry_changes(move |user_id| {
                    if let Err(err) = sender.notify_registry_changed(user_id) {
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
                if let Some((old_connection_id, _)) = self.inner.users.find_connection(user_id).await {
                    if old_connection_id != connection_id {
                        self.publish(HubMessage::Hub(HubEvent::UserDisconnected {
                            user_id,
                            connection_id: old_connection_id,
                        }))
                        .await;
                    }
                }

                self.inner.users.connect(user_id, connection_id, session_key).await;
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
                let Some(removed_connection_id) = self.inner.users.disconnect(user_id, Some(connection_id)).await
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

    async fn create_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubConnectionError> {
        let mut context = self.inner.hub_registry.create_context().await?;
        context.create_connection(user_id, connection_id).await
    }

    async fn remove_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubConnectionError> {
        let mut context = self.inner.hub_registry.create_context().await?;
        context.remove_connection_if_active(user_id, connection_id).await?;
        Ok(())
    }

    pub(crate) async fn heartbeat_registry_connection(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> Result<bool, HubConnectionError> {
        let mut context = self.inner.hub_registry.create_context().await?;
        context.heartbeat_connection(user_id, connection_id).await
    }

    /// Batched heartbeat over all locally-tracked connections on a single pooled connection.
    /// Returns the connections the registry no longer holds as active, which the caller should
    /// disconnect. See [`HubRegistry::heartbeat_connections`] for the reconciliation semantics.
    pub(crate) async fn heartbeat_registry_connections(
        &self,
        connections: &[HubConnection],
    ) -> Result<Vec<HubConnection>, HubConnectionError> {
        let mut context = self.inner.hub_registry.create_context().await?;
        context.heartbeat_connections(connections).await
    }

    async fn process_registry_change(&self, user_id: Uuid) {
        let Some((connection_id, _session_key)) = self.inner.users.find_connection(user_id).await else {
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
        let Some(removed_connection_id) = self.inner.users.disconnect(user_id, Some(connection_id)).await else {
            return;
        };

        self.publish(HubMessage::Hub(HubEvent::UserDisconnected {
            user_id,
            connection_id: removed_connection_id,
        }))
        .await;
    }

    async fn publish(&self, message: HubMessage) {
        self.inner.users.publish(message).await;
    }
}
