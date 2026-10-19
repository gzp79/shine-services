use shine_infra::db::command_log::StreamId;
use uuid::Uuid;

/// Per-stream permission check, called when a stream is opened.
pub trait StreamAccess: Send + Sync + 'static {
    fn can_read(&self, user_id: Uuid, stream_id: &StreamId) -> bool;
    fn can_write(&self, user_id: Uuid, stream_id: &StreamId) -> bool;
}

/// Every authenticated user may read and write every stream.
pub struct AllowAll;

impl StreamAccess for AllowAll {
    fn can_read(&self, _user_id: Uuid, _stream_id: &StreamId) -> bool {
        true
    }

    fn can_write(&self, _user_id: Uuid, _stream_id: &StreamId) -> bool {
        true
    }
}
