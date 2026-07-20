use crate::models::messages::{ToTopic, TopicKey};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum ChatMessage {
    User { user_id: Uuid, text: String },
    System { text: String },
}

impl ToTopic for ChatMessage {
    fn topic(&self) -> TopicKey {
        TopicKey::Chat
    }
}
