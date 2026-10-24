use crate::repositories::command_log::{StoredCommand, StoredSnapshot, StreamId};
use uuid::Uuid;

/// A command as the subscribers see it. It has no inbox position: that stays on the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamCommand {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub command: String,
}

impl From<&StoredCommand> for StreamCommand {
    fn from(command: &StoredCommand) -> Self {
        Self {
            version: command.version,
            command_id: command.command_id,
            user_id: command.user_id,
            command: command.data.clone(),
        }
    }
}

/// The replayed state at a version, as the subscribers get it to start from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamSnapshot {
    pub version: i64,
    pub chain_hash: u64,
    pub state_hash: u64,
    pub data: Vec<u8>,
}

impl From<&StoredSnapshot> for StreamSnapshot {
    fn from(snapshot: &StoredSnapshot) -> Self {
        Self {
            version: snapshot.version,
            chain_hash: snapshot.chain_hash,
            state_hash: snapshot.hash,
            data: snapshot.data.clone(),
        }
    }
}

/// What a stream worker sends to its subscribers. Not tied to any wire format: the socket layer turns
/// it into frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamMessage {
    /// The state to start from: the latest snapshot (none while the stream has no snapshot) and the
    /// commands after it. It replaces whatever the subscriber had of the stream.
    Sync {
        stream_id: StreamId,
        snapshot: Option<StreamSnapshot>,
        commands: Vec<StreamCommand>,
    },
    /// The commands after the last version the subscriber was sent, without gaps.
    Commands {
        stream_id: StreamId,
        commands: Vec<StreamCommand>,
    },
    /// The state hash at a checkpoint version, to compare with the subscriber's own.
    Checkpoint {
        stream_id: StreamId,
        version: i64,
        state_hash: u64,
    },
    /// What the subscriber had of the stream is void; a `Sync` follows.
    Reset { stream_id: StreamId },
    /// The stream cannot be served now. The subscriber is dropped and has to subscribe again.
    Unsynced { stream_id: StreamId },
}

impl StreamMessage {
    pub fn stream_id(&self) -> &StreamId {
        match self {
            StreamMessage::Sync { stream_id, .. }
            | StreamMessage::Commands { stream_id, .. }
            | StreamMessage::Checkpoint { stream_id, .. }
            | StreamMessage::Reset { stream_id }
            | StreamMessage::Unsynced { stream_id } => stream_id,
        }
    }
}
