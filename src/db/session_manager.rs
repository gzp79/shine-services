use crate::db::Identity;
use crate::db::{DBError, DBPool};
use chrono::{DateTime, Duration, Utc};
use redis::{AsyncCommands, Script};
use ring::rand::SystemRandom;
use serde::{Deserialize, Serialize};
use shine_service::service::{CurrentUser, SessionKey, SessionKeyError};
use shine_service::service::{RedisConnectionPool, RedisJsonValue};
use std::sync::Arc;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum DBSessionError {
    #[error("Failed to create session, conflicting keys")]
    KeyConflict,

    #[error(transparent)]
    SessionKeyError(#[from] SessionKeyError),
    #[error(transparent)]
    DBError(#[from] DBError),
}

#[derive(Serialize, Deserialize, Debug, RedisJsonValue)]
#[serde(rename_all = "camelCase")]
struct StoredSession {
    pub session_start: DateTime<Utc>,
    pub name: String,
    pub is_email_confirmed: bool,
}

impl StoredSession {
    fn from_identity(identity: &Identity, session_start: DateTime<Utc>) -> Self {
        Self {
            session_start,
            name: identity.name.clone(),
            is_email_confirmed: identity.is_email_confirmed,
        }
    }

    fn into_current_user(self, user_id: Uuid, session_key: SessionKey) -> CurrentUser {
        CurrentUser {
            is_authentic: true,
            user_id,
            key: session_key,
            session_start: self.session_start,
            name: self.name,
        }
    }
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

    pub async fn create(&self, identity: &Identity) -> Result<CurrentUser, DBSessionError> {
        let created_at = Utc::now();

        let inner = &*self.0;
        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let session_key = SessionKey::new_random(&inner.random)?;
        let key = format!("session:{}:{}", identity.user_id.as_simple(), session_key.to_hex());

        let session = StoredSession::from_identity(identity, created_at);

        let created: bool = client.set_nx(&key, &session).await.map_err(DBError::RedisError)?;
        if created {
            client
                .expire(&key, inner.session_duration)
                .await
                .map_err(DBError::RedisError)?;
            Ok(session.into_current_user(identity.user_id, session_key))
        } else {
            Err(DBSessionError::KeyConflict)
        }
    }

    pub async fn find_session(&self, user_id: Uuid, session_key: SessionKey) -> Result<Option<CurrentUser>, DBError> {
        let inner = &*self.0;
        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let key = format!("session:{}:{}", user_id.as_simple(), session_key.to_hex());
        let session: Option<StoredSession> = client.get(&key).await.map_err(DBError::RedisError)?;
        let session = session.map(|session| session.into_current_user(user_id, session_key));

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
