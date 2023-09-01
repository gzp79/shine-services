use crate::db::DBError;
use crate::db::Identity;
use chrono::{DateTime, Duration, Utc};
use redis::AsyncCommands;
use ring::rand::SystemRandom;
use serde::{Deserialize, Serialize};
use shine_service::service::{ClientFingerprint, CurrentUser, CurrentUserAuthenticity, SessionKey, SessionKeyError};
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
struct SessionSentinel {
    pub start_date: DateTime<Utc>,
    pub fingerprint_hash: String,
}

#[derive(Serialize, Deserialize, Debug, RedisJsonValue)]
#[serde(rename_all = "camelCase")]
struct SessionData {
    pub name: String,
    pub is_email_confirmed: bool,
    pub roles: Vec<String>,
}

#[derive(Debug, ThisError)]
pub enum SessionBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
}

pub struct Inner {
    redis: RedisConnectionPool,
    key_prefix: String,
    ttl_session: usize,
    random: SystemRandom,
}

#[derive(Clone)]
pub struct SessionManager(Arc<Inner>);

impl SessionManager {
    pub async fn new(
        redis: &RedisConnectionPool,
        key_prefix: String,
        ttl_session: Duration,
    ) -> Result<Self, SessionBuildError> {
        Ok(SessionManager(Arc::new(Inner {
            redis: redis.clone(),
            key_prefix,
            random: SystemRandom::new(),
            ttl_session: ttl_session.num_seconds() as usize,
        })))
    }

    fn keys(&self, user_id: Uuid, session_key: &SessionKey) -> (String, String) {
        let inner = &*self.0;
        let prefix = format!(
            "{}session:{}:{}",
            inner.key_prefix,
            user_id.as_simple(),
            session_key.to_hex()
        );
        let sentinel_key = format!("{prefix}:openness");
        let key = format!("{prefix}:data");
        (sentinel_key, key)
    }

    pub async fn create(
        &self,
        identity: &Identity,
        roles: Vec<String>,
        fingerprint: &ClientFingerprint,
    ) -> Result<CurrentUser, DBSessionError> {
        let created_at = Utc::now();

        let inner = &*self.0;

        let session_key = SessionKey::new_random(&inner.random)?;
        let (sentinel_key, key) = self.keys(identity.id, &session_key);

        // Session management in redis:
        // The initial step involves attempting to create a sentinel using a unique key. If this operation 
        // fails, it indicates an exceptionally rare key conflict scenario, and the login process should be
        // restarted (although the likelihood of this occurring is exceedingly low).
        // Once established, this sentinel takes on the responsibility of managing the session's lifespan.
        // It remains immutable and is set to expire after a certain period.
        // Session data is stored within a hash set (hset), where each field corresponds to a different
        // version of the data. A version number signifies that the stored data is no older than the 
        // specified version, though it may be newer due to concurrent updates.
        // To access the current session data, one must retrieve both the sentinel and the data with 
        // the latest version. If either of them has expired or is missing, the session is considered 
        // expired. For instance, during a logout (when the session is deleted), it is possible for a 
        // concurrent update to occur, leading to the removal of the sentinel while new session data has
        // been recently stored. The requirement for both sentinel and data ensures that the session 
        // cannot be extended beyond the default period. Nevertheless, this situation may result in
        // lingering session data, but the expiration mechanism guarantees their eventual deletion.

        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let sentinel = SessionSentinel {
            start_date: created_at,
            fingerprint_hash: fingerprint.hash(),
        };
        let created = client
            .set_nx(&sentinel_key, &sentinel)
            .await
            .map_err(DBError::RedisError)?;
        if created {
            let data = SessionData {
                name: identity.name.clone(),
                is_email_confirmed: identity.is_email_confirmed,
                roles,
            };
            redis::pipe()
                .expire(&sentinel_key, inner.ttl_session)
                .hset_nx(&key, format!("{}", identity.version), &data)
                .expire(&key, inner.ttl_session)
                .query_async::<_, ()>(&mut *client)
                .await
                .map_err(DBError::RedisError)?;

            Ok(CurrentUser {
                authenticity: CurrentUserAuthenticity::NotValidate,
                user_id: identity.id,
                key: session_key,
                name: data.name,
                roles: data.roles,
                session_start: sentinel.start_date,
                fingerprint_hash: sentinel.fingerprint_hash,
                version: identity.version,
            })
        } else {
            Err(DBSessionError::KeyConflict)
        }
    }

    pub async fn update(
        &self,
        session_key: SessionKey,
        identity: &Identity,
        roles: Vec<String>,
    ) -> Result<Option<CurrentUser>, DBError> {
        let inner = &*self.0;
        let (sentinel_key, key) = self.keys(identity.id, &session_key);

        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let is_open = client.exists(sentinel_key).await.map_err(DBError::RedisError)?;
        if is_open {
            let data = SessionData {
                name: identity.name.clone(),
                is_email_confirmed: identity.is_email_confirmed,
                roles,
            };
            redis::pipe()
                .hset_nx(&key, format!("{}", identity.version), &data)
                .ignore()
                .expire(&key, inner.ttl_session)
                .ignore()
                .query_async::<_, ()>(&mut *client)
                .await
                .map_err(DBError::RedisError)?;

            // Perform a full query as this update may had no effect due to the 'nx' flag.
            self.find(identity.id, session_key).await
        } else {
            // sentinel is gone, session is closed.
            Ok(None)
        }
    }

    pub async fn find(&self, user_id: Uuid, session_key: SessionKey) -> Result<Option<CurrentUser>, DBError> {
        let inner = &*self.0;
        let (sentinel_key, key) = self.keys(user_id, &session_key);
        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        // query sentinel and the available data versions
        let (sentinel, data_versions): (Option<SessionSentinel>, Vec<i32>) = redis::pipe()
            .get(sentinel_key)
            .hkeys(&key)
            .query_async(&mut *client)
            .await
            .map_err(DBError::RedisError)?;

        // check if sentinel is present
        let sentinel = match sentinel {
            Some(sentinel) => sentinel,
            _ => return Ok(None),
        };

        // find the latest data version
        let version = match data_versions.into_iter().max() {
            Some(version) => version,
            _ => return Ok(None),
        };

        // find data. In a very unlikely case data could have been just deleted.
        let data: SessionData = match client
            .hget(&key, format!("{version}"))
            .await
            .map_err(DBError::RedisError)?
        {
            Some(data) => data,
            _ => return Ok(None),
        };

        Ok(Some(CurrentUser {
            authenticity: CurrentUserAuthenticity::NotValidate,
            user_id: user_id,
            key: session_key,
            name: data.name,
            roles: data.roles,
            session_start: sentinel.start_date,
            fingerprint_hash: sentinel.fingerprint_hash,
            version,
        }))
    }

    /// Remove an active session of the given user.
    pub async fn remove(&self, user_id: Uuid, session_key: SessionKey) -> Result<(), DBError> {
        let inner = &*self.0;
        let (sentinel_key, key) = self.keys(user_id, &session_key);
        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;
        client.del(&[sentinel_key, key]).await.map_err(DBError::RedisError)?;
        Ok(())
    }

    /// Remove all the active session of the given user.
    pub async fn remove_all(&self, user_id: Uuid) -> Result<(), DBError> {
        let inner = &*self.0;
        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let pattern = format!("{}session:{}:*", inner.key_prefix, user_id.as_simple());
        //log::debug!("pattern: {pattern}");

        let keys = {
            let mut keys = vec![];
            let mut iter: redis::AsyncIter<String> = client.scan_match(pattern).await.map_err(DBError::RedisError)?;
            while let Some(key) = iter.next_item().await {
                keys.push(key);
            }
            keys
        };

        //log::debug!("deleting keys: {keys:?}");
        if !keys.is_empty() {
            client.del(keys).await.map_err(DBError::RedisError)?;
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "./session_manager_test.rs"]
mod session_manager_test;
