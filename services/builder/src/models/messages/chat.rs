use crate::models::messages::{ToTopic, TopicKey};
use uuid::Uuid;

/// One rendered chat comment in an egress batch. `id` is the ordered stream id, so the client can
/// order and deduplicate under at-least-once delivery.
#[derive(Clone, Debug)]
pub struct ChatComment {
    pub id: String,
    pub user_id: Uuid,
    pub text: String,
}

/// A batch of chat comments delivered to a single connection. Egress-only: built by the chat
/// dispatcher and sent via targeted `send_to_connection`, never broadcast.
#[derive(Clone, Debug)]
pub struct ChatBatch {
    pub comments: Vec<ChatComment>,
}

impl ToTopic for ChatBatch {
    fn topic(&self) -> TopicKey {
        TopicKey::Chat
    }
}
