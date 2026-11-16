use crate::replica::ReplayState;
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
pub enum CommandCodecError {
    #[error("command codec error")]
    Postcard(#[from] postcard::Error),
    /// The bytes decode, but are not the canonical encoding of the command they decode to (trailing
    /// bytes, or a non-minimal number).
    #[error("command is not in canonical form")]
    NotCanonical,
}

/// Decodes the postcard bytes of a command of the state `S`. The server and the web client decode
/// commands the same way. Only the canonical encoding is accepted: what decodes is exactly what
/// `encode_command` writes back, so the bytes a client sent are the bytes that are stored and hashed.
pub fn decode_command<S: ReplayState>(bytes: &[u8]) -> Result<S::Command, CommandCodecError> {
    let command: S::Command = postcard::from_bytes(bytes)?;
    if postcard::to_allocvec(&command)? != bytes {
        return Err(CommandCodecError::NotCanonical);
    }
    Ok(command)
}

/// The canonical postcard bytes of a command: the typed command serialized, so the replay is byte for
/// byte deterministic.
pub fn encode_command<S: ReplayState>(command: &S::Command) -> Result<Vec<u8>, CommandCodecError> {
    Ok(postcard::to_allocvec(command)?)
}
