use crate::replay::ReplayState;
use serde::{de::DeserializeOwned, Serialize};

/// A snapshot could not be encoded or decoded.
#[derive(Debug, thiserror::Error)]
#[error("snapshot codec error")]
pub struct SnapshotCodecError(#[from] postcard::Error);

/// The wire encoding of a replayed state, postcard of its serde form: the bytes of `snapshot.data` of a
/// stream frame, identical on the server and in the web client (wasm).
pub fn encode_snapshot<S: Serialize>(state: &S) -> Result<Vec<u8>, SnapshotCodecError> {
    Ok(postcard::to_allocvec(state)?)
}

pub fn decode_snapshot<S: DeserializeOwned>(bytes: &[u8]) -> Result<S, SnapshotCodecError> {
    Ok(postcard::from_bytes(bytes)?)
}

/// A command could not be decoded or encoded.
#[derive(Debug, thiserror::Error)]
#[error("command codec error")]
pub struct CommandCodecError(#[from] serde_json::Error);

/// Decodes the JSON text of a command of the state `S`. The server and the web client decode commands
/// the same way.
pub fn decode_command<S: ReplayState>(text: &str) -> Result<S::Command, CommandCodecError> {
    Ok(serde_json::from_str(text)?)
}

/// The canonical JSON text of a command: the typed command re-serialized, so unknown fields and the
/// field order of the input never reach the log and the replay is byte for byte deterministic.
pub fn encode_command<S: ReplayState>(command: &S::Command) -> Result<String, CommandCodecError> {
    Ok(serde_json::to_string(command)?)
}
