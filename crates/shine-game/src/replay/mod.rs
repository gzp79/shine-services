//! Deterministic command replay shared by the servers and the client.

mod chat;
mod client_stream;
mod codec;
mod envelope;
pub mod protocol;
mod replica;
mod stable_hash;
mod state;

pub use self::{
    chat::{ChatCommand, ChatMessage, ChatState, CHAT_MAX_TEXT_BYTES, CHAT_WINDOW},
    client_stream::{ClientEvent, ClientStream, FrameOutcome, SyncStatus},
    codec::{decode_command, decode_snapshot, encode_command, encode_snapshot, CommandCodecError, SnapshotCodecError},
    envelope::{Checkpoint, CommandContext, CommandEnvelope},
    replica::{ApplyOutcome, Gap, Replica},
    stable_hash::{stable_hash_of, StableHash, StableHasher},
    state::ReplayState,
};
