use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Version of the ws protocol, announced in the first frame of every connection. Bumped when a
/// frame changes in a way a client of the previous version cannot ignore.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum WSMessageRequest {
    Chat { text: String },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatWireComment {
    pub id: String,
    pub from: Uuid,
    pub text: String,
}

/// Why the server rejected a client frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WsErrorCode {
    /// Not a frame of the protocol, or an empty chat text.
    Invalid,
    /// The chat text exceeds the size limit.
    TooLong,
    /// Chat messages arrive faster than the limit allows.
    RateLimited,
    /// The message could not be stored; it was not delivered and may be sent again.
    AppendFailed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[serde(tag = "type")]
pub enum WSMessageResponse {
    /// First frame of a connection.
    Hello { version: u32, connection_id: Uuid },
    /// Comments of the room, in stream order. Ids are ordered, clients drop the ones they have seen.
    Chat { messages: Vec<ChatWireComment> },
    /// The client's chat message was stored under this id.
    Ack { id: String },
    /// The client's last frame was rejected.
    Error { code: WsErrorCode },
}
