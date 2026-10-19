use shine_infra::db::command_log::StreamId;
use std::collections::HashSet;
use uuid::Uuid;

/// Per-stream permission check, called when a stream is opened.
pub trait StreamAccess: Send + Sync + 'static {
    fn can_read(&self, user_id: Uuid, stream_id: &StreamId) -> bool;
    fn can_write(&self, user_id: Uuid, stream_id: &StreamId) -> bool;
}

/// Every authenticated user may read and write every stream.
#[cfg(test)]
pub struct AllowAll;

#[cfg(test)]
impl StreamAccess for AllowAll {
    fn can_read(&self, _user_id: Uuid, _stream_id: &StreamId) -> bool {
        true
    }

    fn can_write(&self, _user_id: Uuid, _stream_id: &StreamId) -> bool {
        true
    }
}

/// Every authenticated user may read and write the listed streams and no other.
pub struct AllowList(HashSet<&'static str>);

impl AllowList {
    pub fn new(streams: impl IntoIterator<Item = &'static str>) -> Self {
        Self(streams.into_iter().collect())
    }
}

impl StreamAccess for AllowList {
    fn can_read(&self, _user_id: Uuid, stream_id: &StreamId) -> bool {
        self.0.contains(stream_id.as_str())
    }

    fn can_write(&self, _user_id: Uuid, stream_id: &StreamId) -> bool {
        self.0.contains(stream_id.as_str())
    }
}
