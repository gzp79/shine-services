use tokio::sync::broadcast;
use uuid::Uuid;

use super::Message;

#[derive(Debug, Clone)]
pub enum MessageSource {
    User(Uuid),
    System,
}

#[derive(Clone)]
pub struct MessageSender {
    source: MessageSource,
    message_channel: broadcast::Sender<Message>,
}

impl MessageSender {
    pub fn new(message_channel: broadcast::Sender<Message>, source: MessageSource) -> Self {
        Self {
            source,
            message_channel,
        }
    }

    pub fn send(&self, message: Message) {
        if let Err(err) = self.message_channel.send(message) {
            log::error!("[{:#?}] Failed to send message: {:#?}", self.source, err);
        }
    }
}
