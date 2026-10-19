use serde::{de::DeserializeOwned, Serialize};

/// A snapshot could not be encoded or decoded.
#[derive(Debug, thiserror::Error)]
#[error("snapshot codec error")]
pub struct SnapshotCodecError(#[from] postcard::Error);

/// Compact binary form (postcard) of a replayed state, as stored in the command log.
pub fn encode_snapshot<S: Serialize>(state: &S) -> Result<Vec<u8>, SnapshotCodecError> {
    Ok(postcard::to_allocvec(state)?)
}

pub fn decode_snapshot<S: DeserializeOwned>(bytes: &[u8]) -> Result<S, SnapshotCodecError> {
    Ok(postcard::from_bytes(bytes)?)
}
