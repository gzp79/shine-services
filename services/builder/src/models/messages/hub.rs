use crate::models::messages::{ChatMessage, HubEvent};
use shine_infra::session::SessionKey;
use uuid::Uuid;

/// High level filter for messages sent to the hub bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TopicKey {
    Hub,
    Chat,
}

pub trait ToTopic {
    fn topic(&self) -> TopicKey;
}

/// Wrapper for messages sent to the hub bus.
#[derive(Clone, Debug)]
pub enum HubCommand {
    ConnectUser { user_id: Uuid, session_key: SessionKey },
    DisconnectUser { user_id: Uuid, session_key: SessionKey },
    Shutdown,

    Chat(ChatMessage),
}

/// Wrapper for messages received from the hub bus.
#[derive(Clone, Debug)]
pub enum HubMessage {
    Hub(HubEvent),
    Chat(ChatMessage),
}

impl ToTopic for HubMessage {
    fn topic(&self) -> TopicKey {
        match self {
            HubMessage::Hub(event) => event.topic(),
            HubMessage::Chat(msg) => msg.topic(),
        }
    }
}
