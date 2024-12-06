use crate::repositories::{DBError, Identity, Role};
use chrono::{DateTime, Duration, Utc};
use redis::AsyncCommands;
use ring::{digest, rand::SystemRandom};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::SiteInfo,
    service::{ClientFingerprint, CurrentUser, RedisConnectionPool, RedisJsonValue, SessionKey, SessionKeyError},
};
use std::sync::Arc;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum DBSessionError {
    #[error("Failed to create session, conflicting keys")]
    KeyConflict,
    #[error("Error in the stored key")]
    InvalidRedisKeyUser(#[source] uuid::Error),

    #[error(transparent)]
    SessionKeyError(#[from] SessionKeyError),
    #[error(transparent)]
    DBError(#[from] DBError),
}

#[derive(Serialize, Deserialize, Debug, RedisJsonValue)]
#[serde(rename_all = "camelCase")]
pub struct SessionSentinel {
    pub created_at: DateTime<Utc>,
    pub fingerprint: String,
    pub agent: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, RedisJsonValue)]
#[serde(rename_all = "camelCase")]
struct SessionData {
    pub name: String,
    pub is_email_confirmed: bool,
    pub roles: Vec<Role>,
}

#[derive(Debug, ThisError)]
pub enum SessionBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
}

pub struct Inner {
    redis: RedisConnectionPool,
    key_prefix: String,
    ttl_session: i64,
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
            ttl_session: ttl_session.num_seconds(),
        })))
    }

    fn to_redis_keys(&self, user_id: Uuid, session_key_hash: &str) -> (String, String) {
        let inner = &*self.0;
        let prefix = format!(
            "{}session:{}:{}",
            inner.key_prefix,
            user_id.as_simple(),
            session_key_hash
        );
        let sentinel_key = format!("{prefix}:openness");
        let key = format!("{prefix}:data");
        (sentinel_key, key)
    }

    fn parse_redis_key(&self, key: &str) -> Result<(Uuid, String), DBSessionError> {
        let inner = &*self.0;
        let user_and_key = match key.strip_prefix(&format!("{}session:", inner.key_prefix)) {
            Some(user_and_key) => user_and_key,
            None => {
                return Err(DBSessionError::from(SessionKeyError::KeyError(
                    "Invalid key format".into(),
                )))
            }
        };

        // pattern: [prefix]session:user:key:[data|openness]
        let mut parts = user_and_key.split(':');
        let user = parts
            .next()
            .ok_or_else(|| SessionKeyError::KeyError("Invalid key format".into()))?;
        let user = Uuid::parse_str(user).map_err(DBSessionError::InvalidRedisKeyUser)?;

        let key = parts
            .next()
            .ok_or_else(|| SessionKeyError::KeyError("Invalid key format".into()))?
            .to_owned();

        if parts.next() != Some("data") {
            return Err(DBSessionError::from(SessionKeyError::KeyError(
                "Invalid key format".into(),
            )));
        }

        if parts.next().is_some() {
            return Err(DBSessionError::from(SessionKeyError::KeyError(
                "Invalid key format".into(),
            )));
        }
        Ok((user, key))
    }

    async fn find_key_hashes(&self, user_id: Uuid) -> Result<Vec<String>, DBSessionError> {
        let inner = &*self.0;
        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let pattern = format!("{}session:{}:*", inner.key_prefix, user_id.as_simple());
        //log::debug!("pattern: {pattern}");

        let mut keys = vec![];
        let mut iter: redis::AsyncIter<String> = client.scan_match(pattern).await.map_err(DBError::RedisError)?;
        while let Some(key) = iter.next_item().await {
            keys.push(key);
        }
        Ok(keys)
    }

    async fn find_by_hash(
        &self,
        user_id: Uuid,
        session_key_hash: String,
    ) -> Result<Option<(SessionSentinel, i32, SessionData)>, DBError> {
        let inner = &*self.0;
        let (sentinel_key, key) = self.to_redis_keys(user_id, &session_key_hash);
        log::debug!(
            "Finding session, user:[{}], sentinel: [{sentinel_key}], data:[{key}]",
            user_id
        );

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
        let data: Option<SessionData> = client
            .hget(&key, format!("{version}"))
            .await
            .map_err(DBError::RedisError)?;
        match data {
            Some(data) => Ok(Some((sentinel, version, data))),
            None => Ok(None),
        }
    }

    async fn update_by_hash(
        &self,
        session_key_hash: String,
        identity: &Identity,
        roles: &[Role],
    ) -> Result<bool, DBSessionError> {
        let inner = &*self.0;
        let (sentinel_key, key) = self.to_redis_keys(identity.id, &session_key_hash);
        log::debug!(
            "Updating session, user:[{}], sentinel: [{sentinel_key}], data:[{key}]",
            identity.id
        );

        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let is_open = client.exists(sentinel_key).await.map_err(DBError::RedisError)?;
        if is_open {
            let data = SessionData {
                name: identity.name.clone(),
                is_email_confirmed: identity.is_email_confirmed,
                roles: roles.to_vec(),
            };
            redis::pipe()
                .hset_nx(&key, format!("{}", identity.version), &data)
                .ignore()
                // it extends to session ttl, but the sentinel is still there to limit the session
                .expire(&key, inner.ttl_session)
                .ignore()
                .query_async::<()>(&mut *client)
                .await
                .map_err(DBError::RedisError)?;
            Ok(true)
        } else {
            // sentinel is gone, session is closed.
            Ok(false)
        }
    }

    #[allow(clippy::wrong_self_convention)]

    pub async fn create(
        &self,
        identity: &Identity,
        roles: Vec<Role>,
        fingerprint: &ClientFingerprint,
        site_info: &SiteInfo,
    ) -> Result<CurrentUser, DBSessionError> {
        let created_at = Utc::now();

        let inner = &*self.0;

        let session_key = SessionKey::new_random(&inner.random)?;
        let session_key_hash = hash_key(&session_key);
        let (sentinel_key, key) = self.to_redis_keys(identity.id, &session_key_hash);
        log::debug!(
            "Storing session, user:[{}], sentinel: [{sentinel_key}], data:[{key}]",
            identity.id
        );

        // Session management in redis:
        // The initial step involves attempting to create a sentinel using a unique key. If this operation
        // fails, it indicates an exceptionally rare key conflict scenario, and the login process should be
        // restarted (although the likelihood of this occurring is extremely low). 
        // Once created, this sentinel takes on the responsibility of managing the session's lifespan.
        // It remains immutable having some expiration time.
        // Session data is stored within a hash set (hset), where each field corresponds to a different
        // version of the data. A version number signifies that the stored data is not older than the
        // specified version, though it may be newer due to concurrent updates.
        // To access the current session data, one must retrieve both the sentinel and the data with
        // the latest version. If either of them has expired or is missing, the session is considered
        // expired. For instance, during a logout (when the session is deleted), it is possible to have a 
        // concurrent update resulting in a removed sentinel, but new session data is still present.
        // So having both the sentinel and the data ensures that the session
        // cannot be extended beyond the default period. Nevertheless, this situation may result in
        // lingering session data, but the expiration mechanism guarantees their eventual deletion.

        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

        let sentinel = SessionSentinel {
            created_at,
            fingerprint: fingerprint.to_string(),
            agent: site_info.agent.clone(),
            country: site_info.country.clone(),
            region: site_info.region.clone(),
            city: site_info.city.clone(),
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
                .query_async::<()>(&mut *client)
                .await
                .map_err(DBError::RedisError)?;

            Ok(CurrentUser {
                user_id: identity.id,
                key: session_key,
                name: data.name,
                roles: data.roles,
                session_start: sentinel.created_at,
                fingerprint: sentinel.fingerprint,
                version: identity.version,
            })
        } else {
            Err(DBSessionError::KeyConflict)
        }
    }

    /// Update a session of a user.
    pub async fn update(
        &self,
        session_key: SessionKey,
        identity: &Identity,
        roles: &[Role],
    ) -> Result<Option<CurrentUser>, DBSessionError> {
        let session_key_hash = hash_key(&session_key);

        if self.update_by_hash(session_key_hash, identity, roles).await? {
            self.find(identity.id, session_key).await
        } else {
            Ok(None)
        }
    }

    /// Update all the session of a user. This is not an atomic operation, if new
    /// sessions are created they are not touched.
    pub async fn update_all(&self, identity: &Identity, roles: &[Role]) -> Result<(), DBSessionError> {
        let keys = self.find_key_hashes(identity.id).await?;

        for key in keys {
            if key.ends_with("data") {
                // we care only for the data keys
                let (user, session_key_hash) = self.parse_redis_key(&key)?;
                assert_eq!(user, identity.id);
                self.update_by_hash(session_key_hash, identity, roles).await?;
            }
        }

        Ok(())
    }

    /// Get all the active session of the given user.
    pub async fn find_all(&self, user_id: Uuid) -> Result<Vec<SessionSentinel>, DBSessionError> {
        let keys = self.find_key_hashes(user_id).await?;

        let inner = &*self.0;
        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;
        let mut sentinels = vec![];

        for key in keys {
            if key.ends_with("openness") {
                let sentinel: Option<SessionSentinel> = client.get(key).await.map_err(DBError::RedisError)?;
                if let Some(sentinel) = sentinel {
                    sentinels.push(sentinel);
                }
            }
        }

        Ok(sentinels)
    }

    pub async fn find(&self, user_id: Uuid, session_key: SessionKey) -> Result<Option<CurrentUser>, DBSessionError> {
        let session_key_hash = hash_key(&session_key);

        match self.find_by_hash(user_id, session_key_hash).await? {
            Some((sentinel, version, data)) => Ok(Some(CurrentUser {
                user_id,
                key: session_key,
                name: data.name,
                roles: data.roles,
                session_start: sentinel.created_at,
                fingerprint: sentinel.fingerprint,
                version,
            })),
            None => Ok(None),
        }
    }

    /// Remove an active session of the given user.
    pub async fn remove(&self, user_id: Uuid, session_key: SessionKey) -> Result<(), DBError> {
        let session_key_hash = hash_key(&session_key);

        let inner = &*self.0;
        let (sentinel_key, key) = self.to_redis_keys(user_id, &session_key_hash);
        log::debug!(
            "Removing session, user:[{}], sentinel: [{sentinel_key}], data:[{key}]",
            user_id
        );

        let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;
        client.del(&[sentinel_key, key]).await.map_err(DBError::RedisError)?;
        Ok(())
    }

    /// Remove all the active session of the given user.
    pub async fn remove_all(&self, user_id: Uuid) -> Result<(), DBSessionError> {
        let keys = self.find_key_hashes(user_id).await?;

        if !keys.is_empty() {
            let inner = &*self.0;
            let mut client = inner.redis.get().await.map_err(DBError::RedisPoolError)?;

            log::debug!("Removing session, user:[{user_id}], keys: {keys:?}");
            client.del(keys).await.map_err(DBError::RedisError)?;
        }

        Ok(())
    }
}

/// Generate a (crypto) hashed version of a session key to protect data in rest.
fn hash_key(key: &SessionKey) -> String {
    // there is no need for a complex hash as key has a big entropy already
    // and it'd be too expensive to invert the hashing.
    let hash = digest::digest(&digest::SHA256, key.as_bytes());
    let hash = hex::encode(hash);
    log::debug!("Hashing session key: {key:?} -> [{hash}]");
    hash
}

#[cfg(test)]
#[path = "./session_manager_test.rs"]
mod session_manager_test;
