use crate::{
    models::ChatError,
    repositories::chat_comments::{redis::RedisChatCommentsDb, ChatCommentDb, ChatCommentStore, StoredChatComment},
};
use uuid::Uuid;

const GLOBAL_ROOM_ID: &str = "global";

#[derive(Clone)]
pub struct ChatService {
    chat_db: RedisChatCommentsDb,
    /// The single room this service reads and writes. Production maps all chat to one global room;
    /// tests override it so a stream is not shared across parallel cases.
    room_id: String,
}

impl ChatService {
    pub fn new(chat_db: RedisChatCommentsDb) -> Self {
        Self::with_room(chat_db, GLOBAL_ROOM_ID)
    }

    pub fn with_room(chat_db: RedisChatCommentsDb, room_id: impl Into<String>) -> Self {
        Self {
            chat_db,
            room_id: room_id.into(),
        }
    }

    pub async fn append_comment(&self, user_id: Uuid, text: &str) -> Result<String, ChatError> {
        let mut context = self.chat_db.create_context().await?;
        context.append_comment(&self.room_id, user_id, text).await
    }

    pub async fn list_after(&self, last_stream_id: &str, limit: usize) -> Result<Vec<StoredChatComment>, ChatError> {
        let mut context = self.chat_db.create_context().await?;
        context.list_after(&self.room_id, last_stream_id, limit).await
    }

    /// Registers `handler` to run on every append to this service's room, so a consumer can push
    /// delivery instead of waiting for its next poll. See [`RedisChatCommentsDb::listen_to_room_changes`].
    pub async fn listen_to_comments<F>(&self, handler: F) -> Result<(), ChatError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.chat_db.listen_to_room_changes(&self.room_id, handler).await
    }
}
