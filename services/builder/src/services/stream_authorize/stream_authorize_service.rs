use crate::{models::CommandStreamError, repositories::command_log::StreamId};
use uuid::Uuid;

/// Decides if a stream may be opened and subscribed to.
#[derive(Clone, Debug, Default)]
pub struct StreamAuthorizeService;

impl StreamAuthorizeService {
    pub fn new() -> Self {
        Self
    }

    /// Returns whether the user may use the stream. Every stream is allowed for now.
    pub async fn is_authorized(&self, _user_id: Uuid, _stream_id: &StreamId) -> Result<bool, CommandStreamError> {
        Ok(true)
    }
}
