use shine_infra::db::postgres::PgError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum CommandLogError {
    #[error("Stream not found")]
    StreamNotFound,
    #[error("Too much contention appending to the stream")]
    Contention,
    #[error("Stream kind mismatch: stored {stored}, requested {requested}")]
    KindMismatch { stored: String, requested: String },
    #[error("Drifted from the persisted chain")]
    Diverged,
    #[error(transparent)]
    Db(#[from] PgError),
}
