use crate::{
    models::messages::{HubMessage, StreamMessage},
    services::{
        hub::{
            hub_command::ControlCommand,
            hub_connections::{Connections, DeadConnection},
            hub_control_queue::ControlQueue,
        },
        StreamEgress,
    },
};
use shine_game::replica::StreamId;
use std::sync::Arc;
use uuid::Uuid;

/// The hub's side of [`StreamEgress`] for the stream to send direct messages to connections bypassing the dispatcher.
#[derive(Clone)]
pub struct HubStreamEgress {
    connections: Arc<Connections>,
    control_sender: ControlQueue,
}

impl HubStreamEgress {
    pub fn new(connections: Arc<Connections>, control_sender: ControlQueue) -> Self {
        Self { connections, control_sender }
    }
}

impl StreamEgress for HubStreamEgress {
    async fn send(&self, connection_id: Uuid, message: Arc<StreamMessage>) -> bool {
        match self
            .connections
            .send_to_connection(connection_id, HubMessage::StreamMessage(message))
            .await
        {
            Ok(queued) => queued,
            Err(DeadConnection { user_id, connection_id, reason }) => {
                if self.control_sender.disconnect(user_id, connection_id, reason).is_err() {
                    tracing::warn!("[{connection_id}] Failed to prune a dead stream connection");
                }
                false
            }
        }
    }

    async fn notify_failed(&self, stream_id: &StreamId) {
        if self
            .control_sender
            .send(ControlCommand::StreamFailed { stream_id: stream_id.clone() })
            .is_err()
        {
            tracing::warn!("[{stream_id}] Failed to queue the stream failure");
        }
    }
}
