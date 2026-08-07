use crate::models::messages::{HubMessage, TopicKey};
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
    },
    DropConnection {
        connection_id: Uuid,
    },
    HubRegistryChanged {
        user_id: Uuid,
    },
    Shutdown,
}
