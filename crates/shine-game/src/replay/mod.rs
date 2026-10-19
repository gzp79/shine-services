//! Deterministic command replay shared by the servers and the client.

mod chat;
mod codec;
mod envelope;
mod policy;
mod replica;
mod stable_hash;
mod state;

pub use self::{
    chat::{ChatCommand, ChatMessage, ChatState, CHAT_MAX_TEXT_BYTES, CHAT_WINDOW},
    codec::{decode_snapshot, encode_snapshot, SnapshotCodecError},
    envelope::{Checkpoint, CommandContext, CommandEnvelope},
    policy::CheckpointPolicy,
    replica::{ApplyOutcome, Gap, Replica},
    stable_hash::{stable_hash_of, StableHash, StableHasher},
    state::ReplayState,
};
