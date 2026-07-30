use crate::{
    models::{
        messages::{HubMessage, Workload},
        HubError,
    },
    services::hub::hub_command::ControlCommand,
};
use shine_infra::session::SessionKey;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Sends operations to the hub. Control commands use an unbounded channel (never lost to
/// back-pressure); payloads use a bounded channel and error when full.
#[derive(Clone)]
pub struct HubSender {
    control_tx: mpsc::UnboundedSender<ControlCommand>,
    payload_tx: mpsc::Sender<Workload>,
}

impl HubSender {
    pub(super) fn new(control_tx: mpsc::UnboundedSender<ControlCommand>, payload_tx: mpsc::Sender<Workload>) -> Self {
        Self { control_tx, payload_tx }
    }

    fn send_control(&self, command: ControlCommand) -> Result<(), HubError> {
        self.control_tx.send(command).map_err(|_| HubError::SendCommandFailed)
    }

    /// Registers a new connection for a user and returns its hub-issued connection id.
    /// Creating a new connection invalidates any prior connection for the user.
    pub fn connect(&self, user_id: Uuid, session_key: SessionKey) -> Result<Uuid, HubError> {
        let connection_id = Uuid::new_v4();
        self.send_control(ControlCommand::ConnectUser {
            user_id,
            connection_id,
            session_key,
        })?;
        Ok(connection_id)
    }

    /// Requests removal of a specific connection. The hub removes it only if `connection_id`
    /// still matches the user's active connection, so a stale request cannot tear down a fresh
    /// reconnect.
    pub fn disconnect(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        self.send_control(ControlCommand::DisconnectUser { user_id, connection_id })
    }

    /// Signals the hub to shut down, publishing a shutdown event to subscribers.
    pub fn shutdown(&self) -> Result<(), HubError> {
        self.send_control(ControlCommand::Shutdown)
    }

    /// Enqueues a registry-change notification to check for the active connection change
    /// for a user.
    pub fn notify_registry_changed(&self, user_id: Uuid) -> Result<(), HubError> {
        self.send_control(ControlCommand::HubRegistryChanged { user_id })
    }

    /// Submits a workload to be broadcast to subscribers, filtered by its topic. Uses the bounded
    /// payload channel and errors when full.
    pub fn send_workload<W: Into<Workload>>(&self, workload: W) -> Result<(), HubError> {
        self.payload_tx
            .try_send(workload.into())
            .map_err(|_| HubError::SendCommandFailed)
    }
}

/// Topic-filtered receiver. Filtering happens on the hub's send side (see ConnectedUsers).
pub struct HubReceiver {
    rx: mpsc::UnboundedReceiver<HubMessage>,
}

impl HubReceiver {
    pub fn new(rx: mpsc::UnboundedReceiver<HubMessage>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<HubMessage> {
        self.rx.recv().await
    }
}
