use crate::models::{
    messages::{HubBusMessage, HubCommand},
    HubError,
};
use tokio::sync::mpsc;

/// Sends commands to the hub.
#[derive(Clone)]
pub struct HubSender {
    command_tx: mpsc::Sender<HubCommand>,
}

impl HubSender {
    pub fn new(command_tx: mpsc::Sender<HubCommand>) -> Self {
        Self { command_tx }
    }

    /// Send a command to the hub.
    pub fn send_command<C: Into<HubCommand>>(&self, command: C) -> Result<(), HubError> {
        let command = command.into();
        self.command_tx
            .try_send(command)
            .map_err(|_| HubError::SendCommandFailed)
    }
}

/// Topic-filtered receiver. Filtering happens on the hub's send side (see ConnectedUsers).
pub struct HubReceiver {
    rx: mpsc::Receiver<HubBusMessage>,
}

impl HubReceiver {
    pub fn new(rx: mpsc::Receiver<HubBusMessage>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<HubBusMessage> {
        self.rx.recv().await
    }
}
