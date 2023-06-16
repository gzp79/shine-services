use crate::db::{DBError, DBPool, RedisConnectionPool, SessionKey, SessionKeyError};
use chrono::{DateTime, Duration, Utc};
use redis::{AsyncCommands, Script};
use ring::rand::SystemRandom;
use serde::{Deserialize, Serialize};
use shine_service::RedisJsonValue;
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

#[derive(Debug)]
pub struct UserSession {
    pub user_id: Uuid,
    pub key: SessionKey,

    pub created_at: DateTime<Utc>,
    //pub client_agent: String,
}
impl UserSession {
    fn from_stored(user_id: Uuid, session_key: SessionKey, session: StoredSession) -> Self {
        Self {
            user_id,
            key: session_key,
            created_at: session.created_at,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, RedisJsonValue)]
struct StoredSession {
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, ThisError)]
pub enum SessionBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
}

pub struct Inner {
    redis: RedisConnectionPool,
    session_duration: usize,
    random: SystemRandom,
}

#[derive(Clone)]
pub struct SessionManager(Arc<Inner>);

impl SessionManager {
    pub async fn new(pool: &DBPool, session_duration: Duration) -> Result<Self, SessionBuildError> {
        Ok(SessionManager(Arc::new(Inner {
            redis: pool.redis.clone(),
            random: SystemRandom::new(),
            session_duration: session_duration.num_seconds() as usize,
        })))
    }

    pub async fn create(&self, user_id: Uuid) -> Result<UserSession, SessionError> {
        let created_at = Utc::now();

        let inner = &*self.0;
        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let session_key = SessionKey::new_random(&inner.random)?;
        let key = format!("session:{}:{}", user_id.as_simple(), session_key.to_hex());

        let session = StoredSession { created_at };

        let created: bool = client.set_nx(&key, &session).await.map_err(DBError::RedisError)?;
        if created {
            client
                .expire(&key, inner.session_duration)
                .await
                .map_err(DBError::RedisError)?;
            Ok(UserSession::from_stored(user_id, session_key, session))
        } else {
            Err(SessionError::KeyConflict)
        }
    }

    pub async fn find_session(&self, user_id: Uuid, session_key: SessionKey) -> Result<Option<UserSession>, DBError> {
        let inner = &*self.0;
        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let key = format!("session:{}:{}", user_id.as_simple(), session_key.to_hex());
        let session: Option<StoredSession> = client.get(&key).await.map_err(DBError::RedisError)?;
        let session = session.map(|session| UserSession::from_stored(user_id, session_key, session));

        Ok(session)
    }

    /// Remove an active session of the given user.
    pub async fn remove(&self, user_id: Uuid, session_key: SessionKey) -> Result<(), DBError> {
        let inner = &*self.0;
        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let key = format!("session:{}:{}", user_id.as_simple(), session_key.to_hex());
        client.del(&key).await.map_err(DBError::RedisError)?;
        Ok(())
    }

    /// Remove all the active session of the given user.
    pub async fn remove_all(&self, user_id: Uuid) -> Result<(), DBError> {
        let inner = &*self.0;
        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let lua_script = r#"
local keys = redis.call('KEYS', ARGV[1] .. '*')
for _, key in ipairs(keys) do
    redis.call('DEL', key)
end
"#;

        let key_prefix = format!("session:{}", user_id.as_simple());
        Script::new(lua_script)
            .arg(key_prefix)
            .invoke_async(&mut *client)
            .await
            .map_err(DBError::RedisError)?;

        Ok(())
    }
}
