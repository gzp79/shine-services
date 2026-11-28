use async_trait::async_trait;
use shine_game::replica::StreamId;
use std::collections::HashSet;
use uuid::Uuid;

/// Decides which streams of a kind a user may open. It is made with the kind and checked when a
/// connection opens a stream, before anything is created for it.
#[async_trait]
pub trait StreamAccess: Send + Sync {
    /// Whether the user may open the stream.
    async fn allows(&self, user_id: Uuid, stream_id: &StreamId) -> bool;
}

/// Every stream may be opened.
pub struct AnyStreamAccess;

#[async_trait]
impl StreamAccess for AnyStreamAccess {
    async fn allows(&self, _user_id: Uuid, _stream_id: &StreamId) -> bool {
        true
    }
}

/// Only the streams of the list may be opened.
pub struct AllowListStreamAccess {
    streams: HashSet<StreamId>,
}

impl AllowListStreamAccess {
    pub fn new(streams: HashSet<StreamId>) -> Self {
        Self { streams }
    }
}

#[async_trait]
impl StreamAccess for AllowListStreamAccess {
    async fn allows(&self, _user_id: Uuid, stream_id: &StreamId) -> bool {
        self.streams.contains(stream_id)
    }
}

#[cfg(test)]
#[path = "stream_access_test.rs"]
mod stream_access_test;
