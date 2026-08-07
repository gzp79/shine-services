use crate::{
    models::ChatError,
    repositories::chat_comments::{redis::RedisChatCommentsDb, ChatCommentDb, ChatCommentStore, StoredChatComment},
};
use uuid::Uuid;

const GLOBAL_ROOM_ID: &str = "global";

#[derive(Clone)]
pub struct ChatService {
    chat_db: RedisChatCommentsDb,
}

impl ChatService {
    pub fn new(chat_db: RedisChatCommentsDb) -> Self {
        Self { chat_db }
    }

    /// For now all chat messages are mapped to a single global room stream.
    pub async fn append_comment(&self, user_id: Uuid, text: &str) -> Result<String, ChatError> {
        let mut context = self.chat_db.create_context().await?;
        context.append_comment(GLOBAL_ROOM_ID, user_id, text).await
    }

    pub async fn list_after(&self, last_stream_id: &str, limit: usize) -> Result<Vec<StoredChatComment>, ChatError> {
        let mut context = self.chat_db.create_context().await?;
        context.list_after(GLOBAL_ROOM_ID, last_stream_id, limit).await
    }
}
