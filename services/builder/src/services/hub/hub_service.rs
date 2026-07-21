use crate::{
    models::messages::{HubCommand, HubEvent, HubMessage, TopicKey},
    repositories::hub_registry::{
        redis::RedisHubConnectionDb,
        HubConnectionDb, HubConnectionError, HubRegistry,
    },
    services::hub::{
        connected_users::ConnectedUsers,
        hub_connection::{HubReceiver, HubSender},
    },
};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

struct Inner {
    command_tx: mpsc::Sender<HubCommand>,
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
    pub fn new(hub_registry: RedisHubConnectionDb) -> Self {
        let (command_tx, command_rx) = mpsc::channel(128);

        let service = Self {
            inner: Arc::new(Inner {
                command_tx,
                users: ConnectedUsers::new(),
                hub_registry,
            }),
        };

        Self::start(service.clone(), command_rx);
        Self::start_registry_listener(service.clone());
        service
    }

    pub fn sender(&self) -> HubSender {
        HubSender::new(self.inner.command_tx.clone())
    }

    /// Subscribe to a set of topics.
    pub async fn subscribe(&self, topics: Vec<TopicKey>) -> HubReceiver {
        let (tx, rx) = mpsc::channel(32);
        self.inner.users.subscribe(topics, tx).await;
        HubReceiver::new(rx)
    }

    fn start(service: HubService, mut command_rx: mpsc::Receiver<HubCommand>) {
        tokio::spawn(async move {
            while let Some(command) = command_rx.recv().await {
                service.process(command).await;
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

                    if let Err(err) = sender.send_command(HubCommand::HubRegistryChanged { user_id }) {
                        log::error!("[{user_id}] Failed to enqueue hub registry change command: {err:#?}");
                    }
                })
                .await
            {
                log::error!("Failed to listen to hub registry changes: {err:#?}");
            }
        });
    }

    async fn process(&self, command: HubCommand) {
        log::debug!("Processing command: {command:#?}");
        match command {
            HubCommand::ConnectUser { user_id, session_key } => {
                let connection_id = match self.create_registry_connection(user_id).await {
                    Ok(connection_id) => connection_id,
                    Err(err) => {
                        log::error!("[{user_id}] Failed to create hub connection: {err:#?}");
                        return;
                    }
                };
                self.inner.users.connect(user_id, connection_id, session_key).await;
                self.publish(HubMessage::Hub(HubEvent::UserConnected { user_id, session_key }))
                    .await;
            }
            HubCommand::DisconnectUser { user_id, .. } => {
                let Some(connection_id) = self.inner.users.disconnect(user_id).await else {
                    return;
                };

                if let Err(err) = self.remove_registry_connection(user_id, connection_id).await {
                    log::error!("[{user_id}] Failed to remove hub connection: {err:#?}");
                    return;
                }

                self.publish(HubMessage::Hub(HubEvent::UserDisconnected { user_id }))
                    .await;
            }
            HubCommand::HubRegistryChanged { user_id } => {
                self.process_registry_change(user_id).await;
            }
            HubCommand::Shutdown => {
                self.publish(HubMessage::Hub(HubEvent::Shutdown)).await;
            }
            HubCommand::Chat(msg) => {
                self.publish(HubMessage::Chat(msg)).await;
            }
        }
    }

    async fn create_registry_connection(&self, user_id: Uuid) -> Result<Uuid, HubConnectionError> {
        let mut context = self.inner.hub_registry.create_context().await?;
        context.create_connection(user_id).await
    }

    async fn remove_registry_connection(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> Result<(), HubConnectionError> {
        let mut context = self.inner.hub_registry.create_context().await?;
        context.remove_connection_if_active(user_id, connection_id).await?;
        Ok(())
    }

    async fn heartbeat_registry_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<bool, HubConnectionError> {
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

        let Some(removed_connection_id) = self.inner.users.disconnect(user_id).await else {
            return;
        };
        if removed_connection_id != connection_id {
            return;
        }

        self.publish(HubMessage::Hub(HubEvent::UserDisconnected { user_id }))
            .await;
    }

    async fn publish(&self, message: HubMessage) {
        self.inner.users.publish(message).await;
    }
}
