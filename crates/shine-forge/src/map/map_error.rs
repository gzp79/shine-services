use lz4_flex::block::DecompressError;
use rmp_serde::{decode::Error as DecError, encode::Error as EncError};
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum MapError {
    #[error("Failed to decompress layer data")]
    DecompressLayerError(#[source] DecompressError),
    #[error("Failed to load layer with data")]
    LoadLayerDataError(#[source] DecError),
    #[error("Failed to load layer with semantic error: {0}")]
    LoadLayerSemanticError(String),
    #[error("Failed to save layer data")]
    SaveLayerError(#[source] EncError),
}
