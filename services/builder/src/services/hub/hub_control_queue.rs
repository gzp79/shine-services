use crate::{
    models::{messages::DisconnectReason, HubError},
    services::hub::{hub_command::ControlCommand, hub_metrics::hub_metrics},
};
use opentelemetry::trace::SpanContext;
use shine_infra::telemetry::current_span_context;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::sync::mpsc;
use uuid::Uuid;

/// A queued command with the span it was queued from and the time it was queued.
pub type ControlMessage = (ControlCommand, SpanContext, Instant);

/// The queue of the hub's command loop, sender side. Every producer goes through it, so the depth is
/// counted in one place.
#[derive(Clone)]
pub struct ControlQueue {
    tx: mpsc::UnboundedSender<ControlMessage>,
    /// Connections with a `DropConnection` queued and not yet processed.
    pending_drops: Arc<Mutex<HashSet<Uuid>>>,
}

impl ControlQueue {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ControlMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let queue = Self {
            tx,
            pending_drops: Arc::new(Mutex::new(HashSet::new())),
        };
        (queue, rx)
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

    /// Queues the removal of a connection found dead. A dead connection is found again by every send to it
    /// until the loop removes it, so a request is queued only if there is none pending for the connection.
    pub fn drop_connection(&self, connection_id: Uuid, reason: DisconnectReason) -> Result<(), HubError> {
        if !self.pending_drops.lock().unwrap().insert(connection_id) {
            hub_metrics().drop_deduplicated();
            return Ok(());
        }
        let result = self.send(ControlCommand::DropConnection { connection_id, reason });
        if result.is_err() {
            self.drop_processed(connection_id);
        }
        result
    }

    /// The loop took the drop request of the connection; a new one may be queued from now on.
    pub fn drop_processed(&self, connection_id: Uuid) {
        self.pending_drops.lock().unwrap().remove(&connection_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queued(rx: &mut mpsc::UnboundedReceiver<ControlMessage>) -> usize {
        std::iter::from_fn(|| rx.try_recv().ok()).count()
    }

    #[test]
    fn a_dead_connection_has_one_drop_queued_at_a_time() {
        let (queue, mut rx) = ControlQueue::new();
        let id = Uuid::new_v4();

        queue.drop_connection(id, DisconnectReason::SlowConsumer).unwrap();
        queue.drop_connection(id, DisconnectReason::SlowConsumer).unwrap();
        assert_eq!(queued(&mut rx), 1);

        queue.drop_processed(id);
        queue.drop_connection(id, DisconnectReason::SlowConsumer).unwrap();
        assert_eq!(queued(&mut rx), 1);
    }

    #[test]
    fn drops_of_other_connections_are_not_deduplicated() {
        let (queue, mut rx) = ControlQueue::new();

        queue
            .drop_connection(Uuid::new_v4(), DisconnectReason::SlowConsumer)
            .unwrap();
        queue
            .drop_connection(Uuid::new_v4(), DisconnectReason::SlowConsumer)
            .unwrap();
        assert_eq!(queued(&mut rx), 2);
    }

    #[test]
    fn a_failed_send_does_not_block_the_next_drop() {
        let (queue, rx) = ControlQueue::new();
        let id = Uuid::new_v4();
        drop(rx);

        assert!(queue.drop_connection(id, DisconnectReason::SlowConsumer).is_err());
        assert!(queue.drop_connection(id, DisconnectReason::SlowConsumer).is_err());
    }
}
