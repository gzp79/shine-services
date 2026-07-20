use crate::models::messages::{ToTopic, TopicKey};
use shine_infra::session::SessionKey;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub enum DisconnectReason {
    SessionExpired,
    ClientClosed,
}

#[derive(Clone, Debug)]
pub enum UserEvent {
    UserConnected { user_id: Uuid, session_key: SessionKey },
    UserDisconnected { user_id: Uuid, reason: DisconnectReason },
}

impl ToTopic for UserEvent {
    fn topic(&self) -> TopicKey {
        TopicKey::UserEvent
    }
}
