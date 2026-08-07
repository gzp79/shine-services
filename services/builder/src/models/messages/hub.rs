use crate::models::messages::{ChatMessage, HubEvent};

/// High level filter for messages sent to the hub bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TopicKey {
    Hub,
    Chat,
}

pub trait ToTopic {
    fn topic(&self) -> TopicKey;
}

/// Wrapper for messages received from the hub bus. Carries both hub-generated lifecycle
/// events and workloads that are broadcast verbatim (see [`Workload`]).
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
