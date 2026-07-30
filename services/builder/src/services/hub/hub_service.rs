use crate::{
    models::messages::{HubEvent, HubMessage, TopicKey, Workload},
    repositories::hub_registry::{redis::RedisHubConnectionDb, HubConnectionDb, HubConnectionError, HubRegistry},
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
use tokio::sync::mpsc;
use uuid::Uuid;

/// Cadence configuration for the hub's periodic connection consumers.
pub struct HubIntervals {
    pub heartbeat: Duration,
    pub session_check: Duration,
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
    pub fn new(
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

        Self::start_dispatcher(service.clone(), control_rx, payload_rx);
        Self::start_registry_listener(service.clone());
        Self::start_heartbeat(service.clone(), intervals.heartbeat);
        Self::start_session_checker(service.clone(), session_service, intervals.session_check);
        service
    }

    pub fn sender(&self) -> HubSender {
        HubSender::new(self.inner.control_tx.clone(), self.inner.payload_tx.clone())
    }

    /// Subscribe to a set of topics.
    pub async fn subscribe(&self, topics: Vec<TopicKey>) -> HubReceiver {
        let (tx, rx) = mpsc::unbounded_channel();
        self.inner.users.subscribe(topics, tx).await;
        HubReceiver::new(rx)
    }

    /// Starts the periodic registry heartbeat on its own connection loop, refreshing TTLs for
    /// locally-tracked connections and disconnecting any the registry no longer holds as active.
    fn start_heartbeat(service: HubService, interval: Duration) {
        tokio::spawn(async move {
            let subscription = service.subscribe(vec![TopicKey::Hub]).await;
            let task = HeartbeatTask::new(service);
            run_connection_loop(subscription, interval, task).await;
        });
    }

    /// Starts the periodic session checker on its own connection loop, validating each tracked
    /// session and issuing a targeted disconnect on expiry.
    fn start_session_checker(service: HubService, session_service: Arc<CurrentUserService>, interval: Duration) {
        tokio::spawn(async move {
            let subscription = service.subscribe(vec![TopicKey::Hub]).await;
            let checker = SessionChecker::new(session_service, &service);
            run_connection_loop(subscription, interval, checker).await;
        });
    }

    /// Starts the central dispatch loop that drains the control and payload channels, biased so
    /// lifecycle signals are handled ahead of broadcast payloads.
    fn start_dispatcher(
        service: HubService,
        mut control_rx: mpsc::UnboundedReceiver<ControlCommand>,
        mut payload_rx: mpsc::Receiver<Workload>,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Control is prioritized so lifecycle signals are handled promptly.
                    biased;
                    control = control_rx.recv() => match control {
                        Some(command) => service.process(command).await,
                        None => break,
                    },
                    payload = payload_rx.recv() => match payload {
                        // Broadcast verbatim; conversion is a pure move with no hub processing.
                        Some(workload) => service.publish(workload.into()).await,
                        None => break,
                    },
                }
            }
        });
    }

    fn start_registry_listener(service: HubService) {
        tokio::spawn(async move {
            let sender = service.sender();
            if let Err(err) = service
                .inner
                .hub_registry
                .listen_to_registry_changes(move |payload| {
                    let user_id = match Uuid::parse_str(payload) {
                        Ok(user_id) => user_id,
                        Err(err) => {
                            log::error!("Failed to parse hub registry payload {payload:?}: {err:#?}");
                            return;
                        }
                    };

                    if let Err(err) = sender.notify_registry_changed(user_id) {
                        log::error!("[{user_id}] Failed to enqueue hub registry change command: {err:#?}");
                    }
                })
                .await
            {
                log::error!("Failed to listen to hub registry changes: {err:#?}");
            }
        });
    }

    async fn process(&self, command: ControlCommand) {
        log::debug!("Processing command: {command:#?}");
        match command {
            ControlCommand::ConnectUser {
                user_id,
                connection_id,
                session_key,
            } => {
                // If a different connection is currently active for this user, tear it down
                // explicitly before recording the replacement, so the old socket closes and the
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

                if let Err(err) = self.create_registry_connection(user_id, connection_id).await {
                    log::error!("[{user_id}] Failed to create hub connection: {err:#?}");
                    return;
                }
                self.inner.users.connect(user_id, connection_id, session_key).await;
                self.publish(HubMessage::Hub(HubEvent::UserConnected {
                    user_id,
                    connection_id,
                    session_key,
                }))
                .await;
            }
            ControlCommand::DisconnectUser { user_id, connection_id } => {
                // Only removes the entry when `connection_id` still matches the active
                // connection, so a stale disconnect can never tear down a fresh reconnect.
                let Some(removed_connection_id) = self.inner.users.disconnect(user_id, Some(connection_id)).await
                else {
                    return;
                };

                if let Err(err) = self.remove_registry_connection(user_id, removed_connection_id).await {
                    log::error!("[{user_id}] Failed to remove hub connection: {err:#?}");
                    return;
                }

                self.publish(HubMessage::Hub(HubEvent::UserDisconnected {
                    user_id,
                    connection_id: removed_connection_id,
                }))
                .await;
            }
            ControlCommand::HubRegistryChanged { user_id } => {
                self.process_registry_change(user_id).await;
            }
            ControlCommand::Shutdown => {
                self.publish(HubMessage::Hub(HubEvent::Shutdown)).await;
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
