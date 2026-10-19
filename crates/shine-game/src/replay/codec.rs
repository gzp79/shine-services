use crate::replay::ReplayState;

#[derive(Debug, thiserror::Error)]
#[error("snapshot codec error")]
pub struct SnapshotCodecError(#[from] postcard::Error);

pub fn encode_snapshot<S: ReplayState>(state: &S) -> Result<Vec<u8>, SnapshotCodecError> {
    Ok(postcard::to_allocvec(state)?)
}

pub fn decode_snapshot<S: ReplayState>(bytes: &[u8]) -> Result<S, SnapshotCodecError> {
    Ok(postcard::from_bytes(bytes)?)
}
