use crate::models::ChatError;
use std::future::Future;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredChatComment {
    pub stream_id: String,
    pub user_id: Uuid,
    pub text: String,
}

/// One read of a room stream.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChatPage {
    /// The readable comments of the page, in stream order.
    pub comments: Vec<StoredChatComment>,
    /// Id of the last entry read, readable or not. A reader resumes after it, so entries that cannot
    /// be parsed are skipped instead of being read again forever. `None` when nothing was read.
    pub last_id: Option<String>,
    /// The page was full, so more entries may follow.
    pub more: bool,
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
    ) -> impl Future<Output = Result<ChatPage, ChatError>> + Send;

    /// Returns the id of the newest entry of the room, `None` for an empty or missing stream.
    fn last_id(&mut self, room_id: &str) -> impl Future<Output = Result<Option<String>, ChatError>> + Send;
}
