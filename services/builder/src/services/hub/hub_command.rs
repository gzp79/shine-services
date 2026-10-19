use crate::models::messages::{DisconnectReason, HubMessage, TopicKey};
use shine_infra::session::SessionKey;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Control messages processed by the hub's command loop — the sole writer of connection state.
#[derive(Debug)]
pub enum ControlCommand {
    ConnectUser {
        user_id: Uuid,
        connection_id: Uuid,
        session_key: SessionKey,
        tx: mpsc::Sender<HubMessage>,
        topics: Vec<TopicKey>,
    },
    DisconnectUser {
        user_id: Uuid,
        connection_id: Uuid,
        reason: DisconnectReason,
    },
    DropConnection {
        connection_id: Uuid,
        reason: DisconnectReason,
    },
    HubRegistryChanged {
        user_id: Uuid,
    },
    Shutdown,
}

impl ControlCommand {
    /// Short name for tracing.
    pub fn name(&self) -> &'static str {
        match self {
            ControlCommand::ConnectUser { .. } => "connect_user",
            ControlCommand::DisconnectUser { .. } => "disconnect_user",
            ControlCommand::DropConnection { .. } => "drop_connection",
            ControlCommand::HubRegistryChanged { .. } => "hub_registry_changed",
            ControlCommand::Shutdown => "shutdown",
        }
    }

    /// User the command is about, for tracing.
    pub fn user_id(&self) -> Option<Uuid> {
        match self {
            ControlCommand::ConnectUser { user_id, .. }
            | ControlCommand::DisconnectUser { user_id, .. }
            | ControlCommand::HubRegistryChanged { user_id } => Some(*user_id),
            ControlCommand::DropConnection { .. } | ControlCommand::Shutdown => None,
        }
    }

    /// Connection the command is about, for tracing.
    pub fn connection_id(&self) -> Option<Uuid> {
        match self {
            ControlCommand::ConnectUser { connection_id, .. }
            | ControlCommand::DisconnectUser { connection_id, .. }
            | ControlCommand::DropConnection { connection_id, .. } => Some(*connection_id),
            ControlCommand::HubRegistryChanged { .. } | ControlCommand::Shutdown => None,
        }
    }
}
