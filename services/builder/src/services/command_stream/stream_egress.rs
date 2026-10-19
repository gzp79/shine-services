use crate::models::messages::StreamMessage;
use futures::future::BoxFuture;
use uuid::Uuid;

/// Where the stream workers send their frames: the hub, in production.
pub trait StreamEgress: Send + Sync + 'static {
    /// Queues a message for a connection. `false` when it was refused (the connection is gone or too slow):
    /// the worker must send nothing more to it, a later frame would leave a gap.
    fn send(&self, connection_id: Uuid, message: StreamMessage) -> BoxFuture<'_, bool>;
}
