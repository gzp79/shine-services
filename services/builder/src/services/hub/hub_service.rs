use crate::{
    models::{
        messages::{CommandFailure, DisconnectReason, HubMessage, StreamMessage, SubscribeFailure, TopicKey},
        CommandStreamError, HubError,
    },
    repositories::{
        command_inbox::redis::RedisCommandInboxDb, command_log::pg::PgCommandLogDb,
        hub_registry::redis::RedisHubConnectionDb,
    },
    services::{
        command_stream::{stream_metrics, StreamRegistry},
        hub::{
            control_command::ControlCommand,
            hub_connections::Connections,
            hub_gateway::{ControlMessage, HubGateway},
            hub_loop::HubLoop,
            hub_registry_service::HubRegistryService,
            hub_subscribers::Subscribers,
            socket_channel::SocketSender,
            tasks::{run_background_task, BackgroundJoinHandler, Heartbeat, SessionChecker, StreamDispatcher},
        },
        StreamEgress,
    },
};
use serde::{Deserialize, Serialize};
use shine_core::tokio::TaskSupervisor;
use shine_game::replica::StreamId;
use shine_infra::{
    serde::{consts, duration},
    session::{CurrentUserService, SessionKey},
    sync::{Admission, AdmissionError, AdmissionPermit, RateLimit},
};
use std::{sync::Arc, time::Duration};
use thiserror::Error as ThisError;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};
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
    gateway: HubGateway,
    control_receiver: mpsc::UnboundedReceiver<ControlMessage>,

    session_service: Option<Arc<CurrentUserService>>,
    hub_registry: Option<RedisHubConnectionDb>,
    stream_registry: Option<StreamRegistry<RedisCommandInboxDb, PgCommandLogDb>>,
    supervisor: TaskSupervisor,
}

impl HubServiceBuilder {
    fn new(config: HubConfig) -> Self {
        let connections = Arc::new(Connections::new());
        let (gateway, control_receiver) = HubGateway::new(connections);
        Self {
            config,
            gateway,
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
        F: FnOnce(&HubGateway) -> Result<StreamRegistry<RedisCommandInboxDb, PgCommandLogDb>, CommandStreamError>,
    {
        self.stream_registry = Some(stream_registry(&self.gateway)?);
        Ok(self)
    }

    pub async fn build(self) -> Result<HubService, HubServiceBuilderError> {
        let Self {
            config,
            gateway,
            control_receiver,
            session_service,
            hub_registry,
            stream_registry,
            supervisor,
        } = self;

        let session_service = session_service.ok_or(HubServiceBuilderError::MissingSessionService)?;
        let hub_registry = hub_registry.ok_or(HubServiceBuilderError::MissingHubRegistry)?;
        let hub_registry = HubRegistryService::new(hub_registry);
        let subscribers = Subscribers::new();

        // All consumer subscriptions must be registered before the command loop starts dispatching: the
        // consumers are event-sourced, so a lifecycle event dispatched before they subscribe is lost to them
        // forever. The spawn_* calls subscribe synchronously (awaited), and the loop is started last so nothing
        // drains the channels until every subscription is in place.
        let mut background_tasks = vec![
            BackgroundJoinHandler::abortable(
                "hub registry listener",
                Self::start_registry_listener(hub_registry.clone(), gateway.clone(), supervisor.clone()),
            ),
            run_background_task(
                &subscribers,
                &supervisor,
                config.heartbeat,
                None,
                Heartbeat::new(hub_registry.clone(), gateway.clone()),
            )
            .await,
            run_background_task(
                &subscribers,
                &supervisor,
                config.session_check,
                None,
                SessionChecker::new(session_service, gateway.clone()),
            )
            .await,
        ];
        if let Some(registry) = stream_registry.clone() {
            registry.listen().await?;
            background_tasks.push(BackgroundJoinHandler::run_to_end(
                "hub stream dispatcher",
                StreamDispatcher::start(
                    &subscribers,
                    &supervisor,
                    gateway.clone(),
                    registry,
                    config.stream_shutdown_timeout,
                )
                .await,
            ));
        }

        let shutdown_complete = HubLoop::start(
            gateway.clone(),
            control_receiver,
            subscribers.clone(),
            hub_registry.clone(),
            stream_registry.clone(),
            supervisor.clone(),
            background_tasks,
        );

        Ok(HubService {
            inner: Arc::new(Inner {
                gateway,
                shutdown_complete,
                subscribers,
                stream_registry,
                ping_enabled: config.ping_enabled,
                admission: Admission::new(config.connect_rate, config.max_connections),
            }),
        })
    }

    fn start_registry_listener(
        hub_registry: HubRegistryService,
        gateway: HubGateway,
        supervisor: TaskSupervisor,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(err) = hub_registry
                .listen_to_changes(move |user_id| {
                    if let Err(err) = gateway.send_control(ControlCommand::HubRegistryChanged { user_id }) {
                        tracing::warn!("[{user_id}] Failed to enqueue hub registry change command: {err:#?}");
                    }
                })
                .await
            {
                tracing::error!("Failed to listen to hub registry changes: {err:#?}");
                supervisor.task_ended("hub registry listener");
            }
        })
    }
}

struct Inner {
    gateway: HubGateway,
    shutdown_complete: watch::Receiver<bool>,
    subscribers: Subscribers,
    stream_registry: Option<StreamRegistry<RedisCommandInboxDb, PgCommandLogDb>>,
    ping_enabled: bool,
    admission: Admission<Uuid>,
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

    /// Admits a connect of the user, before its socket is upgraded. The permit is held for as long as the
    /// socket is served.
    pub fn admit(&self, user_id: Uuid) -> Result<AdmissionPermit, AdmissionError> {
        self.inner.admission.admit(user_id)
    }

    /// Subscribes to a set of topics on the hub's message bus.
    #[cfg(test)]
    pub async fn subscribe(&self, topics: Vec<TopicKey>) -> mpsc::UnboundedReceiver<HubMessage> {
        self.inner.subscribers.subscribe(topics).await
    }

    /// Stops the hub: asks the command loop to shut down and returns once it has. The error is the failed send of
    /// the command, which means that the loop was gone already; the hub has stopped when this returns, either way.
    pub async fn shutdown(&self) -> Result<(), HubError> {
        let sent = self.send_control(ControlCommand::Shutdown);
        self.wait_shutdown_complete().await;
        sent
    }

    /// Waits until the command loop has stopped.
    pub(crate) async fn wait_shutdown_complete(&self) {
        let mut shutdown_complete = self.inner.shutdown_complete.clone();
        while !*shutdown_complete.borrow_and_update() {
            if shutdown_complete.changed().await.is_err() {
                return;
            }
        }
    }

    /// Live connection and subscriber counts for status reporting.
    pub async fn stats(&self) -> HubStats {
        let connections = self.inner.gateway.connection_count().await;
        let subscribers = self.inner.subscribers.subscription_count().await;
        HubStats { connections, subscribers }
    }
}

/// Hub messaging operations
impl HubService {
    /// Requests a new connection for a user, superseding any prior one, and returns its id.
    pub fn request_connect(
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

    /// Requests the end of a connection, only if `connection_id` still matches the user's active one. `Client`
    /// is a disconnect the client asked for; any other reason is a drop by the hub, counted once the connection is
    /// actually removed.
    pub fn request_disconnect(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
        reason: DisconnectReason,
    ) -> Result<(), HubError> {
        self.send_control(ControlCommand::DisconnectUser { user_id, connection_id, reason })
    }

    /// Sends an egress message to a specific connection. Returns whether the message was queued for delivery.
    #[cfg(test)]
    #[tracing::instrument(level = "debug", skip_all, fields(ws.connection_id = %connection_id))]
    pub async fn send_to_connection(&self, connection_id: Uuid, message: HubMessage) -> Result<bool, HubError> {
        self.inner.gateway.send_to_connection(connection_id, message).await
    }

    /// Sends an egress message to a user's active connection.
    #[tracing::instrument(level = "debug", skip_all, fields(enduser.id = %user_id))]
    pub async fn send_to_user(&self, user_id: Uuid, message: HubMessage) -> Result<(), HubError> {
        self.inner.gateway.send_to_user(user_id, message).await
    }

    /// Broadcasts an egress message to every connection subscribed to its topic.
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn broadcast(&self, message: HubMessage) -> Result<(), HubError> {
        self.inner.gateway.broadcast(message).await
    }

    pub(super) fn send_control(&self, command: ControlCommand) -> Result<(), HubError> {
        self.inner.gateway.send_control(command)
    }
}

/// Stream operations for the hub.
impl HubService {
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
        self.send_control(ControlCommand::SubscribeStream { connection_id, stream_id })
            .map_err(|_| SubscribeFailure::Failed)
    }

    pub fn unsubscribe_stream(&self, connection_id: Uuid, stream_id: StreamId) -> Result<(), HubError> {
        if let Some(registry) = &self.inner.stream_registry {
            registry.close(connection_id, &stream_id);
        }
        self.send_control(ControlCommand::UnsubscribeStream { connection_id, stream_id })
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
}

impl StreamEgress for HubService {
    async fn send(&self, connection_id: Uuid, message: Arc<StreamMessage>) -> bool {
        StreamEgress::send(&self.inner.gateway, connection_id, message).await
    }

    async fn notify_failed(&self, stream_id: &StreamId) {
        StreamEgress::notify_failed(&self.inner.gateway, stream_id).await
    }
}
