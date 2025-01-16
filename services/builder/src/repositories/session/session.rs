use std::collections::HashSet;

use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use super::{Message, SessionError};

pub struct Session {
    id: Uuid,
    message_channel: broadcast::Sender<Message>,
    users: RwLock<HashSet<Uuid>>,
}

impl Session {
    pub fn new(id: Uuid) -> Self {
        let (message_channel, _) = broadcast::channel(32);
        Self {
            id,
            message_channel,
            users: RwLock::new(HashSet::new()),
        }
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub async fn connect_user(&self, user_id: Uuid) -> Result<(), SessionError> {
        //todo: check if user_id is allowed to access the session

        {
            let mut users = self.users.write().await;
            // todo: use redis to check if user is already connected in another service instance
            if !users.insert(user_id) {
                return Err(SessionError::UserAlreadyConnected);
            }
        }

        Ok(())
    }

    pub async fn disconnect_user(&self, user_id: Uuid) {
        let mut users = self.users.write().await;
        users.remove(&user_id);
    }

    pub fn message_channel(&self) -> (broadcast::Sender<Message>, broadcast::Receiver<Message>) {
        (self.message_channel.clone(), self.message_channel.subscribe())
    }
}
