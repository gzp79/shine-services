use shine_game::replica::{StreamId, StreamKind};
use uuid::Uuid;

/// Factory of unique stream ids.
pub struct TestStreamId;

impl TestStreamId {
    pub fn new_random(kind: StreamKind) -> StreamId {
        StreamId::new(kind, format!("t-{}", Uuid::new_v4().simple())).unwrap()
    }

    pub fn new_chat() -> StreamId {
        Self::new_random(StreamKind::Chat)
    }

    /// A chat stream id with the given (not necessarily unique) name.
    pub fn new_chat_with(name: &str) -> StreamId {
        StreamId::new(StreamKind::Chat, name).unwrap()
    }

    pub fn new_pixel_map() -> StreamId {
        Self::new_random(StreamKind::PixelMap)
    }
}
