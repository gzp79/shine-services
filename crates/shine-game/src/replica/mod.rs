//! Deterministic command replay shared by the servers and the client.

mod chat;
mod client_stream;
mod codec;
mod pixel_map;
mod replica;
mod stable_hash;
mod state;
mod stream_id;
mod wire;

pub use self::{
    chat::{ChatCommand, ChatMessage, ChatState, CHAT_MAX_TEXT_BYTES, CHAT_WINDOW},
    client_stream::{ClientStream, StreamStatus, SubmitError},
    codec::{decode_command, decode_snapshot, encode_command, encode_snapshot, CommandCodecError, SnapshotCodecError},
    pixel_map::{PixelMapCommand, PixelMapState, PIXEL_MAP_COLORS, PIXEL_MAP_HEIGHT, PIXEL_MAP_WIDTH},
    replica::{ApplyError, ApplyOutcome, Checkpoint, CommandEnvelope, CommandHeader, Replica},
    stable_hash::{fold_chain_hash, stable_hash_of, StableHash, StableHasher},
    state::{Command, ReplayState},
    stream_id::{StreamId, StreamIdError},
    wire::{parse_server_message, ClientMessage, ServerMessage, WireCommand, WireSnapshot},
};
