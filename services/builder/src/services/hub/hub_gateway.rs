use crate::{
    models::{
        messages::{HubMessage, StreamMessage},
        HubError,
    },
    services::{
        hub::{
            control_command::ControlCommand,
            hub_connections::{Connections, FailedSend},
            hub_metrics::hub_metrics,
        },
        StreamEgress,
    },
};
use opentelemetry::trace::SpanContext;
use shine_game::replica::StreamId;
use shine_infra::telemetry::current_span_context;
use std::{sync::Arc, time::Instant};
use tokio::sync::mpsc;
use uuid::Uuid;

/// A queued command with the span it was queued from and the time it was queued.
pub type ControlMessage = (ControlCommand, SpanContext, Instant);

/// Handle message delivery and command requests to the hub.
#[derive(Clone)]
pub struct HubGateway {
    connections: Arc<Connections>,
    control_sender: mpsc::UnboundedSender<ControlMessage>,
}

impl HubGateway {
    /// Creates the gateway over the connections, and the receiving end of its requests for the command loop.
    pub fn new(connections: Arc<Connections>) -> (Self, mpsc::UnboundedReceiver<ControlMessage>) {
        let (control_sender, control_receiver) = mpsc::unbounded_channel();
        (Self { connections, control_sender }, control_receiver)
    }

    pub fn connections(&self) -> &Connections {
        &self.connections
    }

    /// Number of connected users.
    pub async fn connection_count(&self) -> usize {
        self.connections.len().await
    }

    /// Queues a control command for the command loop.
    pub fn send_control(&self, command: ControlCommand) -> Result<(), HubError> {
        let metrics = hub_metrics();
        metrics.control_queued();
        self.control_sender
            .send((command, current_span_context(), Instant::now()))
            .map_err(|_| {
                metrics.control_dequeued();
                HubError::SendCommandFailed
            })
    }

    /// Sends a message to one connection without waiting. Returns whether it was queued.
    pub async fn send_to_connection(&self, connection_id: Uuid, message: HubMessage) -> Result<bool, HubError> {
        self.connections
            .send_to_connection(connection_id, message)
            .await
            .or_else(|failed| self.report_dead([failed]).map(|()| false))
    }

    /// Sends a message to the user's active connection.
    pub async fn send_to_user(&self, user_id: Uuid, message: HubMessage) -> Result<(), HubError> {
        self.report_dead(self.connections.send_to_user(user_id, message).await.err())
    }

    /// Sends a message to every connection subscribed to its topic.
    pub async fn broadcast(&self, message: HubMessage) -> Result<(), HubError> {
        self.report_dead(self.connections.broadcast(message).await.err().unwrap_or_default())
    }

    /// Asks the loop to remove the connections found dead during a send. Only the first send that found a
    /// connection dead gets it here, so a dead connection is requested once.
    fn report_dead(&self, dead: impl IntoIterator<Item = FailedSend>) -> Result<(), HubError> {
        for FailedSend { user_id, connection_id, reason } in dead {
            self.send_control(ControlCommand::DisconnectUser { user_id, connection_id, reason })?;
        }
        Ok(())
    }
}

impl StreamEgress for HubGateway {
    async fn send(&self, connection_id: Uuid, message: Arc<StreamMessage>) -> bool {
        match self
            .send_to_connection(connection_id, HubMessage::StreamMessage(message))
            .await
        {
            Ok(queued) => queued,
            Err(err) => {
                tracing::warn!("[{connection_id}] Failed to send a stream message: {err:#?}");
                false
            }
        }
    }

    async fn notify_failed(&self, stream_id: &StreamId) {
        if let Err(err) = self.send_control(ControlCommand::StreamFailed { stream_id: stream_id.clone() }) {
            tracing::warn!("[{stream_id}] Failed to queue the stream failure: {err:#?}");
        }
    }
}

#[cfg(test)]
#[path = "hub_gateway_test.rs"]
mod hub_gateway_test;
