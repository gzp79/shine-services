use crate::models::messages::{HubBusMessage, HubCommand, TopicKey, UserEvent};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::{
    connected_users::ConnectedUsers,
    hub_connection::{HubReceiver, HubSender},
};

struct Inner {
    command_tx: mpsc::Sender<HubCommand>,
    users: ConnectedUsers,
}

/// Messaging service for connected users and processes.
/// Commands are submitted through a HubSender; subscribers receive events
/// through a topic-filtered BusSubscription.
#[derive(Clone)]
pub struct HubService {
    inner: Arc<Inner>,
}

impl HubService {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel(128);

        let service = Self {
            inner: Arc::new(Inner {
                command_tx,
                users: ConnectedUsers::new(),
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
                self.inner.users.connect(user_id, session_key).await;
                self.publish(HubBusMessage::Hub(UserEvent::UserConnected { user_id, session_key }))
                    .await;
            }
            HubCommand::DisconnectUser { user_id, reason } => {
                if self.inner.users.disconnect(user_id).await {
                    self.publish(HubBusMessage::Hub(UserEvent::UserDisconnected { user_id, reason }))
                        .await;
                }
            }
            HubCommand::Chat(msg) => {
                self.publish(HubBusMessage::Chat(msg)).await;
            }
        }
    }

    async fn publish(&self, message: HubBusMessage) {
        self.inner.users.publish(message).await;
    }
}
