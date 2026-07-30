use crate::models::messages::{ChatMessage, HubMessage, ToTopic, TopicKey};

/// A message submitted to the hub for broadcasting for processing by other services.
/// The hub does not process workloads itself; it simply forwards them to subscribers.
#[derive(Clone, Debug)]
pub enum Workload {
    Chat(ChatMessage),
}

impl ToTopic for Workload {
    fn topic(&self) -> TopicKey {
        match self {
            Workload::Chat(msg) => msg.topic(),
        }
    }
}

impl From<ChatMessage> for Workload {
    fn from(msg: ChatMessage) -> Self {
        Workload::Chat(msg)
    }
}

impl From<Workload> for HubMessage {
    /// Converts a workload into a bus message. This is a pure move — no hub processing.
    fn from(workload: Workload) -> Self {
        match workload {
            Workload::Chat(msg) => HubMessage::Chat(msg),
        }
    }
}
