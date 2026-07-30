use shine_infra::session::SessionKey;
use uuid::Uuid;

/// Control messages processed by the hub's command loop.
#[derive(Clone, Debug)]
pub enum ControlCommand {
    ConnectUser {
        user_id: Uuid,
        connection_id: Uuid,
        session_key: SessionKey,
    },
    DisconnectUser {
        user_id: Uuid,
        connection_id: Uuid,
    },
    HubRegistryChanged {
        user_id: Uuid,
    },
    Shutdown,
}
