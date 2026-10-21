use crate::{
    models::messages::{HubMessage, StreamMessage},
    services::{command_stream::StreamEgress, HubService},
};
use futures::future::BoxFuture;
use uuid::Uuid;

impl StreamEgress for HubService {
    fn send(&self, connection_id: Uuid, message: StreamMessage) -> BoxFuture<'_, bool> {
        Box::pin(async move {
            // A full channel gives `Ok(false)`: the hub drops that connection as a slow consumer, and the
            // worker must stop sending to it.
            match self
                .send_to_connection(connection_id, HubMessage::Stream(message))
                .await
            {
                Ok(queued) => queued,
                Err(err) => {
                    tracing::warn!("[{connection_id}] Failed to send a stream message: {err:#?}");
                    false
                }
            }
        })
    }
}
