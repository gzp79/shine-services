use crate::{
    models::{messages::DisconnectReason, HubError},
    services::hub::{hub_command::ControlCommand, hub_metrics::hub_metrics},
};
use opentelemetry::trace::SpanContext;
use shine_infra::telemetry::current_span_context;
use std::time::Instant;
use tokio::sync::mpsc;
use uuid::Uuid;

/// A queued command with the span it was queued from and the time it was queued.
pub type ControlMessage = (ControlCommand, SpanContext, Instant);

/// The queue of the hub's command loop, sender side.
#[derive(Clone)]
pub struct ControlQueue {
    tx: mpsc::UnboundedSender<ControlMessage>,
}

impl ControlQueue {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ControlMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx }, rx)
    }

    pub fn send(&self, command: ControlCommand) -> Result<(), HubError> {
        let metrics = hub_metrics();
        metrics.control_queued();
        self.tx
            .send((command, current_span_context(), Instant::now()))
            .map_err(|_| {
                metrics.control_dequeued();
                HubError::SendCommandFailed
            })
    }

    /// Queues the removal of a connection found dead. It is queued once per connection: only the send that
    /// closed the connection first reports it.
    pub fn disconnect(&self, user_id: Uuid, connection_id: Uuid, reason: DisconnectReason) -> Result<(), HubError> {
        self.send(ControlCommand::DisconnectUser { user_id, connection_id, reason })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disconnect_is_queued_with_its_reason() {
        let (queue, mut rx) = ControlQueue::new();
        let (user, id) = (Uuid::new_v4(), Uuid::new_v4());

        queue.disconnect(user, id, DisconnectReason::SlowConsumer).unwrap();

        let (command, _, _) = rx.try_recv().unwrap();
        assert!(matches!(
            command,
            ControlCommand::DisconnectUser { user_id, connection_id, reason: DisconnectReason::SlowConsumer }
                if user_id == user && connection_id == id
        ));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn a_failed_send_is_an_error() {
        let (queue, rx) = ControlQueue::new();
        drop(rx);

        assert!(queue
            .disconnect(Uuid::new_v4(), Uuid::new_v4(), DisconnectReason::SlowConsumer)
            .is_err());
    }
}
