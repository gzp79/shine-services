use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum TileMapError {
    #[error("Chunk not found")]
    ChunkNotFound,

    #[cfg(feature = "persisted")]
    #[error(transparent)]
    EventStoreError(#[from] shine_infra::db::event_source::EventStoreError),
}
