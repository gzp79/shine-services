use lz4_flex::block::DecompressError;
use rmp_serde::{decode::Error as DeError, encode::Error as EnError};
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum MapError {
    #[error("Failed to decompress layer data")]
    DecompressLayerError(#[source] DecompressError),
    #[error("Failed to load layer data")]
    LoadLayerError(#[source] DeError),
    #[error("Failed to save layer data")]
    SaveLayerError(#[source] EnError),
}
