use crate::models::messages::{ToTopic, TopicKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PingEvent {
    pub id: String,
}

impl PingEvent {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

impl ToTopic for PingEvent {
    fn topic(&self) -> TopicKey {
        TopicKey::Ping
    }
}
