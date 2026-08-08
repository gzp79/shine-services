use crate::models::ChatError;
use std::future::Future;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredChatComment {
    pub stream_id: String,
    pub user_id: Uuid,
    pub text: String,
}

pub trait ChatCommentStore {
    /// Appends one message to a room stream and returns the Redis stream id cursor.
    fn append_comment(
        &mut self,
        room_id: &str,
        user_id: Uuid,
        text: &str,
    ) -> impl Future<Output = Result<String, ChatError>> + Send;

    /// Returns messages strictly after `last_stream_id`.
    fn list_after(
        &mut self,
        room_id: &str,
        last_stream_id: &str,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<StoredChatComment>, ChatError>> + Send;
}
