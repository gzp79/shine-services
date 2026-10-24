use crate::models::{CommandInboxError, CommandLogError};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum CommandStreamError {
    #[error(transparent)]
    Log(#[from] CommandLogError),
    #[error(transparent)]
    Inbox(#[from] CommandInboxError),
    #[error("Stored stream data is unusable: {0}")]
    Corrupt(String),
}
