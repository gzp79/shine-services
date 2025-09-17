use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum MapError {
    #[error("Failed to load layer data: {0}")]
    LoadLayerFailed(String),
}
