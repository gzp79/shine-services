use crate::db::{DBError, DBPool, RedisConnectionPool, SessionId, SessionIdError};
use ring::rand::{SecureRandom, SystemRandom};
use std::sync::Arc;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum SessionError {
    #[error("Failed to generate session key: {0}")]
    KeyError(String),
    #[error("Failed to create session, conflicting keys")]
    KeyConflict,

    #[error(transparent)]
    SessionIdError(#[from] SessionIdError),
    #[error(transparent)]
    DBError(#[from] DBError),
}

#[derive(Debug, ThisError)]
pub enum SessionBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
}

pub struct Inner {
    redis: RedisConnectionPool,
    random: SystemRandom,
}

#[derive(Clone)]
pub struct SessionManager(Arc<Inner>);

impl SessionManager {
    pub async fn new(pool: &DBPool) -> Result<Self, SessionBuildError> {
        Ok(SessionManager(Arc::new(Inner {
            redis: pool.redis.clone(),
            random: SystemRandom::new(),
        })))
    }

    pub async fn create(&self, user_id: &Uuid) -> Result<SessionId, SessionError> {
        let inner = &*self.0;
        let client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let session_id = {
            let mut raw_id = [0; 32];
            raw_id[..16].copy_from_slice(user_id.as_bytes());
            inner
                .random
                .fill(&mut raw_id[16..])
                .map_err(|err| SessionError::KeyError(format!("{err:#?}")))?;
            SessionId::from_raw(raw_id)
        };

        let key = session_id.to_token();

        //KeyConflict

        Ok(session_id)
    }
}
