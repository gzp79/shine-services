use crate::models::messages::{ToTopic, TopicKey};
use std::sync::Arc;
use uuid::Uuid;

/// One rendered chat comment in an egress batch. `id` is the ordered stream id, so the client can
/// order and deduplicate under at-least-once delivery.
#[derive(Clone, Debug)]
pub struct ChatComment {
    pub id: String,
    pub user_id: Uuid,
    pub text: String,
}

/// A batch of chat comments delivered to connections. Egress-only: built by the chat dispatcher and
/// sent via targeted `send_to_connection`, never broadcast. The comments are shared, so one batch
/// serves every connection that is at the same point of the stream.
#[derive(Clone, Debug)]
pub struct ChatBatch {
    pub comments: Arc<[ChatComment]>,
}

impl ChatBatch {
    pub fn new(comments: Vec<ChatComment>) -> Self {
        Self { comments: comments.into() }
    }
}

impl ToTopic for ChatBatch {
    fn topic(&self) -> TopicKey {
        TopicKey::Chat
    }
}
