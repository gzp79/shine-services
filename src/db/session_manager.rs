use crate::db::{DBError, DBPool, RedisConnectionPool, SessionId, SessionKey, SessionKeyError};
use ring::rand::SystemRandom;
use std::sync::Arc;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum SessionError {
    #[error("Failed to create session, conflicting keys")]
    KeyConflict,

    #[error(transparent)]
    SessionKeyError(#[from] SessionKeyError),
    #[error(transparent)]
    DBError(#[from] DBError),
}

pub struct UserSession {
    id: SessionId,
}

impl UserSession {
    pub fn session_id(&self) -> &SessionId {
        &self.id
    }
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

    pub async fn create(&self, user_id: Uuid) -> Result<UserSession, SessionError> {
        let inner = &*self.0;
        let client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let session_key = SessionKey::new_random(&inner.random)?;
        //KeyConflict

        Ok(UserSession {
            id: SessionId {
                user_id,
                key: session_key,
            },
        })
    }

    pub async fn find_session(&self, user_id: Uuid, key: SessionKey) -> Result<Option<UserSession>, DBError> {
        todo!()
    }

    /// Remove an active session of the given user.
    pub async fn remove(&self, user_id: Uuid, key: SessionKey) -> Result<(), DBError> {
        todo!()
    }

    /// Remove all the active session of the given user.
    pub async fn remove_all(&self, user_id: Uuid) -> Result<(), DBError> {
        todo!()
    }
}
