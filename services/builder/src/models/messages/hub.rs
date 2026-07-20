use crate::models::messages::{ChatMessage, DisconnectReason, UserEvent};
use shine_infra::session::SessionKey;
use uuid::Uuid;

/// High level filter for messages sent to the hub bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TopicKey {
    UserEvent,
    Chat,
}

pub trait ToTopic {
    fn topic(&self) -> TopicKey;
}

/// Wrapper for messages sent to the hub bus.
#[derive(Clone, Debug)]
pub enum HubCommand {
    ConnectUser { user_id: Uuid, session_key: SessionKey },
    DisconnectUser { user_id: Uuid, reason: DisconnectReason },
    Chat(ChatMessage),
}

/// Wrapper for messages received from the hub bus.
#[derive(Clone, Debug)]
pub enum HubBusMessage {
    Hub(UserEvent),
    Chat(ChatMessage),
}

impl ToTopic for HubBusMessage {
    fn topic(&self) -> TopicKey {
        match self {
            HubBusMessage::Hub(event) => event.topic(),
            HubBusMessage::Chat(msg) => msg.topic(),
        }
    }
}
