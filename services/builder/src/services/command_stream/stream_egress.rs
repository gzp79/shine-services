use crate::services::command_stream::StreamMessage;
use std::{future::Future, sync::Arc};
use uuid::Uuid;

/// Where a stream worker sends its messages: to a connection, by its id. The hub implements it; the
/// worker does not know about sockets.
pub trait StreamEgress: Clone + Send + Sync + 'static {
    /// Queues the message for the connection and returns whether it was queued. It never waits for
    /// the connection to take it. `false` means the connection is unknown, gone or too slow to keep
    /// up: the worker forgets it as a subscriber.
    ///
    /// Messages to one connection arrive in the order they were sent.
    fn send(&self, connection_id: Uuid, message: Arc<StreamMessage>) -> impl Future<Output = bool> + Send;
}
