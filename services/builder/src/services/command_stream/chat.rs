//! The chat as a stream: one global room, whose state is `shine_game::replay::ChatState`.

use crate::services::command_stream::AllowList;

/// The id of the global chat room; created by the first subscribe.
pub const CHAT_STREAM_ID: &str = "chat-global";

/// Every authenticated user reads and writes the global chat, and nothing else: chat is the only kind
/// registered, so no other stream may be created.
pub fn chat_access() -> AllowList {
    AllowList::new([CHAT_STREAM_ID])
}

#[cfg(test)]
#[path = "chat_test.rs"]
mod chat_test;
