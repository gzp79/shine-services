use crate::models::messages::{HubMessage, PingEvent};
use shine_game::replica::{encode_command, ChatCommand, ChatState, PixelMapCommand, PixelMapState};

/// Factory of the messages and the encoded stream commands the tests send.
pub struct TestMessage;

impl TestMessage {
    /// A ping message with the given id.
    pub fn new_ping(id: impl Into<String>) -> HubMessage {
        HubMessage::Ping(PingEvent { id: id.into() })
    }

    /// An encoded command adding a chat message.
    pub fn new_chat_command(text: &str) -> String {
        encode_command::<ChatState>(&ChatCommand::Add { text: text.into() }).unwrap()
    }

    /// An encoded command painting a pixel; the position may be off the map.
    pub fn new_paint_command(x: u8, y: u8, color: u8) -> String {
        encode_command::<PixelMapState>(&PixelMapCommand::Paint { x, y, color }).unwrap()
    }
}
