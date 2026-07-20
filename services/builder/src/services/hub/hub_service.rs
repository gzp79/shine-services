use crate::models::messages::{HubCommand, HubEvent, HubMessage, TopicKey};
use crate::repositories::hub_connections::redis::RedisHubConnectionDb;
use crate::repositories::hub_connections::{HubConnectionDb, HubConnections};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::{
    connected_users::ConnectedUsers,
    hub_connection::{HubReceiver, HubSender},
};

struct Inner {
    command_tx: mpsc::Sender<HubCommand>,
    users: ConnectedUsers,
    hub_connection_db: RedisHubConnectionDb,
}

/// Messaging service for connected users and processes.
/// Commands are submitted through a HubSender; subscribers receive events
/// through a topic-filtered BusSubscription.
#[derive(Clone)]
pub struct HubService {
    inner: Arc<Inner>,
}

impl HubService {
    pub fn new(hub_connection_db: RedisHubConnectionDb) -> Self {
        let (command_tx, command_rx) = mpsc::channel(128);

        let service = Self {
            inner: Arc::new(Inner {
                command_tx,
                users: ConnectedUsers::new(),
                hub_connection_db,
            }),
        };

        Self::start(service.clone(), command_rx);
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

    async fn process(&self, command: HubCommand) {
        log::debug!("Processing command: {command:#?}");
        match command {
            HubCommand::ConnectUser { user_id, session_key } => {
                let mut context = match self.inner.hub_connection_db.create_context().await {
                    Ok(context) => context,
                    Err(err) => {
                        log::error!("[{user_id}] Failed to create hub connection db context: {err:#?}");
                        return;
                    }
                };
                let connection_id = match context.create_connection(user_id).await {
                    Ok(connection_id) => connection_id,
                    Err(err) => {
                        log::error!("[{user_id}] Failed to store hub connection: {err:#?}");
                        return;
                    }
                };
                self.inner.users.connect(user_id, connection_id, session_key).await;
                self.publish(HubMessage::Hub(HubEvent::UserConnected { user_id, session_key }))
                    .await;
            }
            HubCommand::DisconnectUser { user_id, session_key } => {
                let Some(connection_id) = self.inner.users.disconnect(user_id, session_key).await else {
                    return;
                };

                let mut context = match self.inner.hub_connection_db.create_context().await {
                    Ok(context) => context,
                    Err(err) => {
                        log::error!("[{user_id}] Failed to create hub connection db context: {err:#?}");
                        return;
                    }
                };

                if let Err(err) = context.remove_connection_if_active(user_id, connection_id).await {
                    log::error!("[{user_id}] Failed to remove hub connection: {err:#?}");
                    return;
                }

                self.publish(HubMessage::Hub(HubEvent::UserDisconnected { user_id }))
                    .await;
            }
            HubCommand::Shutdown => {
                self.publish(HubMessage::Hub(HubEvent::Shutdown)).await;
            }
            HubCommand::Chat(msg) => {
                self.publish(HubMessage::Chat(msg)).await;
            }
        }
    }

    async fn publish(&self, message: HubMessage) {
        self.inner.users.publish(message).await;
    }
}
