use shine_infra::db::DBError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum RedisSessionBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
}
