use crate::models::messages::{ToTopic, TopicKey};
use shine_infra::session::SessionKey;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum HubEvent {
    UserConnected { user_id: Uuid, session_key: SessionKey },
    UserDisconnected { user_id: Uuid },
    Shutdown,
}

impl ToTopic for HubEvent {
    fn topic(&self) -> TopicKey {
        TopicKey::Hub
    }
}
