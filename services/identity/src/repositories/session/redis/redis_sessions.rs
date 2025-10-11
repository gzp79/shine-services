use crate::repositories::{
    identity::Identity,
    session::{Session, SessionError, SessionInfo, SessionUser, Sessions},
};
use chrono::{DateTime, Duration, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use shine_infra::{
    db::{DBError, RedisJsonValue},
    web::extracts::SiteInfo,
};
use uuid::Uuid;

use super::RedisSessionDbContext;

#[derive(Serialize, Deserialize, Debug, RedisJsonValue)]
#[serde(rename_all = "camelCase")]
pub struct RedisSessionSentinel {
    pub created_at: DateTime<Utc>,
    pub fingerprint: String,
    pub agent: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, RedisJsonValue)]
#[serde(rename_all = "camelCase")]
struct RedisSessionUser {
    pub name: String,
    pub is_email_confirmed: bool,
    pub is_linked: bool,
    pub roles: Vec<String>,
}

fn create_session_info(user_id: Uuid, key_hash: String, sentinel: RedisSessionSentinel) -> SessionInfo {
    SessionInfo {
        created_at: sentinel.created_at,
        user_id,
        key_hash,
        fingerprint: sentinel.fingerprint,
        site_info: SiteInfo {
            agent: sentinel.agent,
            country: sentinel.country,
            region: sentinel.region,
            city: sentinel.city,
        },
    }
}

fn create_session(
    user_id: Uuid,
    key_hash: String,
    sentinel: RedisSessionSentinel,
    user_data: RedisSessionUser,
    expire_at: DateTime<Utc>,
) -> Session {
    Session {
        info: create_session_info(user_id, key_hash, sentinel),
        user: SessionUser {
            name: user_data.name,
            is_email_confirmed: user_data.is_email_confirmed,
            is_linked: user_data.is_linked,
            roles: user_data.roles,
        },
        expire_at,
    }
}

impl RedisSessionDbContext<'_> {
    fn to_redis_keys(&self, user_id: Uuid, session_key_hash: &str) -> (String, String) {
        let prefix = format!(
            "{}session:{}:{}",
            self.key_prefix,
            user_id.as_simple(),
            session_key_hash
        );
        let sentinel_key = format!("{prefix}:sentinel");
        let key = format!("{prefix}:data");
        (sentinel_key, key)
    }

    fn parse_redis_key<'k>(&self, key: &'k str) -> Result<(Uuid, &'k str, &'k str), SessionError> {
        let user_and_key = match key.strip_prefix(&format!("{}session:", self.key_prefix)) {
            Some(user_and_key) => user_and_key,
            None => return Err(SessionError::InvalidKey),
        };

        // pattern: [prefix]session:user:key:[data|sentinel]
        let mut parts = user_and_key.split(':');
        let user = parts.next().ok_or(SessionError::InvalidKey)?;
        let user = Uuid::parse_str(user).map_err(|_| SessionError::InvalidKey)?;
        let key = parts.next().ok_or(SessionError::InvalidKey)?;
        let role = parts.next().ok_or(SessionError::InvalidKey)?;
        if !["data", "sentinel"].contains(&role) {
            return Err(SessionError::InvalidKey);
        }
        if parts.next().is_some() {
            return Err(SessionError::InvalidKey);
        }

        Ok((user, key, role))
    }

    async fn find_redis_keys(&mut self, user_id: Uuid) -> Result<Vec<String>, SessionError> {
        let pattern = format!("{}session:{}:*", self.key_prefix, user_id.as_simple());
        //log::debug!("pattern: {pattern}");

        let mut keys = vec![];
        let mut iter = self
            .client
            .scan_match::<String, _>(pattern)
            .await
            .map_err(DBError::RedisError)?;
        while let Some(key) = iter.next_item().await {
            let key = key.map_err(DBError::RedisError)?;
            keys.push(key);
        }
        Ok(keys)
    }
}

impl Sessions for RedisSessionDbContext<'_> {
    async fn store_session(
        &mut self,
        created_at: DateTime<Utc>,
        session_key_hash: String,
        fingerprint: String,
        site_info: &SiteInfo,
        identity: &Identity,
        roles: Vec<String>,
        is_linked: bool,
    ) -> Result<Session, SessionError> {
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
        // It remains immutable having some expiration time. The expiration time is extended on each access.
        // Session data is stored within a different key. To access the current session data, one must retrieve
        // both the sentinel and the data. If either of them has expired or is missing, the session is considered
        // expired. For instance, during a logout (when the session is deleted), it is possible to have a
        // concurrent update resulting in a removed sentinel, but a new session data is still present.
        // So having both the sentinel and the data ensures that the session
        // cannot be extended beyond the default period. Nevertheless, this situation may result in
        // lingering session data, but the expiration mechanism guarantees their eventual deletion.

        let sentinel = RedisSessionSentinel {
            created_at,
            fingerprint: fingerprint.to_string(),
            agent: site_info.agent.clone(),
            country: site_info.country.clone(),
            region: site_info.region.clone(),
            city: site_info.city.clone(),
        };

        log::debug!("sentinel:{sentinel:#?}");
        let created = self
            .client
            .set_nx(&sentinel_key, &sentinel)
            .await
            .map_err(DBError::RedisError)?;
        if created {
            let data = RedisSessionUser {
                name: identity.name.clone(),
                is_email_confirmed: identity.is_email_confirmed,
                is_linked,
                roles,
            };
            log::debug!("data:{sentinel:#?}");
            redis::pipe()
                .expire(&sentinel_key, self.ttl_session)
                .set(&key, &data)
                .expire(&key, self.ttl_session)
                .query_async::<()>(&mut *self.client)
                .await
                .map_err(DBError::RedisError)?;

            Ok(Session {
                info: SessionInfo {
                    created_at,
                    user_id: identity.id,
                    key_hash: session_key_hash,
                    fingerprint: sentinel.fingerprint,
                    site_info: site_info.clone(),
                },
                user: SessionUser {
                    name: data.name,
                    is_email_confirmed: data.is_email_confirmed,
                    is_linked: data.is_linked,
                    roles: data.roles,
                },
                expire_at: Utc::now() + Duration::seconds(self.ttl_session),
            })
        } else {
            log::debug!("key conflict");
            Err(SessionError::KeyConflict)
        }
    }

    async fn find_all_session_hashes_by_user(&mut self, user_id: Uuid) -> Result<Vec<String>, SessionError> {
        let keys = self.find_redis_keys(user_id).await?;

        let mut key_hashes = vec![];

        for key in keys {
            let (key_user_id, key_session_key_hash, key_role) = self.parse_redis_key(&key)?;
            assert_eq!(key_user_id, user_id);
            if key_role == "data" {
                // we care only for the data keys
                key_hashes.push(key_session_key_hash.to_owned());
            }
        }

        Ok(key_hashes)
    }

    async fn find_all_sessions_by_user(&mut self, user_id: Uuid) -> Result<Vec<Session>, SessionError> {
        let keys = self.find_redis_keys(user_id).await?;

        let mut sessions = vec![];

        for key in keys {
            let (key_user_id, key_session_key_hash, key_role) = self.parse_redis_key(&key)?;
            assert_eq!(key_user_id, user_id);
            if key_role == "sentinel" {
                if let Some(session) = self.find_session_by_hash(user_id, key_session_key_hash).await? {
                    sessions.push(session);
                }
            }
        }

        Ok(sessions)
    }

    async fn find_session_by_hash(
        &mut self,
        user_id: Uuid,
        session_key_hash: &str,
    ) -> Result<Option<Session>, SessionError> {
        let (sentinel_key, key) = self.to_redis_keys(user_id, session_key_hash);
        log::debug!("Finding session, user:[{user_id}], sentinel: [{sentinel_key}], data:[{key}]");

        // query sentinel and the available data versions
        let (sentinel, sentinel_ttl, data, data_ttl): (
            Option<RedisSessionSentinel>,
            Option<i64>,
            Option<RedisSessionUser>,
            Option<i64>,
        ) = redis::pipe()
            .get(&sentinel_key)
            .ttl(&sentinel_key)
            .get(&key)
            .ttl(&key)
            .query_async(&mut *self.client)
            .await
            .map_err(DBError::RedisError)?;

        let (sentinel, sentinel_ttl) = match (sentinel, sentinel_ttl) {
            (Some(sentinel), Some(sentinel_ttl)) => (sentinel, sentinel_ttl),
            _ => return Ok(None),
        };
        let (data, data_ttl) = match (data, data_ttl) {
            (Some(data), Some(data_ttl)) => (data, data_ttl),
            _ => return Ok(None),
        };

        let ttl = sentinel_ttl.min(data_ttl);
        let expire_at = Utc::now() + Duration::seconds(ttl);

        Ok(Some(create_session(
            user_id,
            session_key_hash.to_string(),
            sentinel,
            data,
            expire_at,
        )))
    }

    async fn update_session_user_by_hash(
        &mut self,
        session_key_hash: &str,
        identity: &Identity,
        roles: &[String],
        is_linked: bool,
    ) -> Result<Option<Session>, SessionError> {
        let (sentinel_key, key) = self.to_redis_keys(identity.id, session_key_hash);
        log::debug!(
            "Updating session, user:[{}], sentinel: [{sentinel_key}], data:[{key}]",
            identity.id
        );

        let is_open = self.client.exists(sentinel_key).await.map_err(DBError::RedisError)?;
        if is_open {
            // an update on the session extends the expiration time
            let data = RedisSessionUser {
                name: identity.name.clone(),
                is_email_confirmed: identity.is_email_confirmed,
                is_linked,
                roles: roles.to_vec(),
            };
            redis::pipe()
                .expire(&key, self.ttl_session)
                .set(&key, &data)
                .expire(&key, self.ttl_session)
                .ignore()
                .query_async::<()>(&mut *self.client)
                .await
                .map_err(DBError::RedisError)?;
            self.find_session_by_hash(identity.id, session_key_hash).await
        } else {
            // sentinel is gone, session is closed.
            Ok(None)
        }
    }

    async fn delete_session_by_hash(&mut self, user_id: Uuid, session_key_hash: &str) -> Result<(), SessionError> {
        let (sentinel_key, key) = self.to_redis_keys(user_id, session_key_hash);
        log::debug!("Removing session, user:[{user_id}], sentinel: [{sentinel_key}], data:[{key}]");

        // todo: https://github.com/redis-rs/redis-rs/issues/1228, https://github.com/redis-rs/redis-rs/issues/1322
        () = self
            .client
            .del(&[sentinel_key, key])
            .await
            .map_err(DBError::RedisError)?;
        Ok(())
    }

    async fn delete_all_sessions_by_user(&mut self, user_id: Uuid) -> Result<(), SessionError> {
        let keys = self.find_redis_keys(user_id).await?;

        if !keys.is_empty() {
            log::debug!("Removing session, user:[{user_id}], keys: {keys:?}");
            // todo: https://github.com/redis-rs/redis-rs/issues/1228, https://github.com/redis-rs/redis-rs/issues/1322
            () = self.client.del(keys).await.map_err(DBError::RedisError)?;
        }

        Ok(())
    }
}
