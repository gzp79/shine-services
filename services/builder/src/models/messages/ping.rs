use crate::models::messages::{ToTopic, TopicKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PingEvent {
    pub id: String,
}

impl ToTopic for PingEvent {
    fn topic(&self) -> TopicKey {
        TopicKey::Ping
    }
}
