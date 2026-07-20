use crate::models::messages::{ToTopic, TopicKey};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub user_id: Uuid,
    pub text: String,
}

impl ToTopic for ChatMessage {
    fn topic(&self) -> TopicKey {
        TopicKey::Chat
    }
}
