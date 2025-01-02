use crate::repositories::{
    identity::Identity,
    session::{Session, SessionError, SessionInfo, SessionUser, Sessions},
    DBError,
};
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use shine_core::{db::RedisJsonValue, web::SiteInfo};
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
    user_version: i32,
    user_data: RedisSessionUser,
) -> Session {
    Session {
        info: create_session_info(user_id, key_hash, sentinel),
        user_version,
        user: SessionUser {
            name: user_data.name,
            roles: user_data.roles,
        },
    }
}

impl<'a> RedisSessionDbContext<'a> {
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
        let mut iter: redis::AsyncIter<String> = self.client.scan_match(pattern).await.map_err(DBError::RedisError)?;
        while let Some(key) = iter.next_item().await {
            keys.push(key);
        }
        Ok(keys)
    }
}

impl<'a> Sessions for RedisSessionDbContext<'a> {
    async fn store_session(
        &mut self,
        created_at: DateTime<Utc>,
        session_key_hash: String,
        fingerprint: String,
        site_info: &SiteInfo,
        identity: &Identity,
        roles: Vec<String>,
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

        let sentinel = RedisSessionSentinel {
            created_at,
            fingerprint: fingerprint.to_string(),
            agent: site_info.agent.clone(),
            country: site_info.country.clone(),
            region: site_info.region.clone(),
            city: site_info.city.clone(),
        };
        log::debug!("sentinel:{:#?}", sentinel);
        let created = self
            .client
            .set_nx(&sentinel_key, &sentinel)
            .await
            .map_err(DBError::RedisError)?;
        if created {
            let data = RedisSessionUser {
                name: identity.name.clone(),
                is_email_confirmed: identity.is_email_confirmed,
                roles,
            };
            log::debug!("data:{:#?}", sentinel);
            redis::pipe()
                .expire(&sentinel_key, self.ttl_session)
                .hset_nx(&key, format!("{}", identity.version), &data)
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
                user_version: identity.version,
                user: SessionUser {
                    name: data.name,
                    roles: data.roles,
                },
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

    async fn find_all_session_infos_by_user(&mut self, user_id: Uuid) -> Result<Vec<SessionInfo>, SessionError> {
        let keys = self.find_redis_keys(user_id).await?;

        let mut sessions = vec![];

        for key in keys {
            let (key_user_id, key_session_key_hash, key_role) = self.parse_redis_key(&key)?;
            assert_eq!(key_user_id, user_id);
            if key_role == "sentinel" {
                let sentinel: Option<RedisSessionSentinel> =
                    self.client.get(&key).await.map_err(DBError::RedisError)?;
                if let Some(sentinel) = sentinel {
                    sessions.push(create_session_info(
                        key_user_id,
                        key_session_key_hash.to_owned(),
                        sentinel,
                    ));
                }
            }
        }

        Ok(sessions)
    }

    async fn find_session_by_hash(
        &mut self,
        user_id: Uuid,
        session_key_hash: String,
    ) -> Result<Option<Session>, SessionError> {
        let (sentinel_key, key) = self.to_redis_keys(user_id, &session_key_hash);
        log::debug!(
            "Finding session, user:[{}], sentinel: [{sentinel_key}], data:[{key}]",
            user_id
        );

        // query sentinel and the available data versions
        let (sentinel, data_versions): (Option<RedisSessionSentinel>, Vec<i32>) = redis::pipe()
            .get(sentinel_key)
            .hkeys(&key)
            .query_async(&mut *self.client)
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

        // find data
        let data: Option<RedisSessionUser> = self
            .client
            .hget(&key, format!("{version}"))
            .await
            .map_err(DBError::RedisError)?;
        match data {
            // In a very unlikely case, data could have been deleted.
            None => Ok(None),
            Some(data) => Ok(Some(create_session(user_id, session_key_hash, sentinel, version, data))),
        }
    }

    async fn update_session_user_by_hash(
        &mut self,
        session_key_hash: String,
        identity: &Identity,
        roles: &[String],
    ) -> Result<Option<Session>, SessionError> {
        let (sentinel_key, key) = self.to_redis_keys(identity.id, &session_key_hash);
        log::debug!(
            "Updating session, user:[{}], sentinel: [{sentinel_key}], data:[{key}]",
            identity.id
        );

        let is_open = self.client.exists(sentinel_key).await.map_err(DBError::RedisError)?;
        if is_open {
            let data = RedisSessionUser {
                name: identity.name.clone(),
                is_email_confirmed: identity.is_email_confirmed,
                roles: roles.to_vec(),
            };
            redis::pipe()
                .hset_nx(&key, format!("{}", identity.version), &data)
                .ignore()
                // it extends to session ttl, but the sentinel is still there to limit the session
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

    async fn delete_session_by_hash(&mut self, user_id: Uuid, session_key_hash: String) -> Result<(), SessionError> {
        let (sentinel_key, key) = self.to_redis_keys(user_id, &session_key_hash);
        log::debug!(
            "Removing session, user:[{}], sentinel: [{sentinel_key}], data:[{key}]",
            user_id
        );

        self.client
            .del(&[sentinel_key, key])
            .await
            .map_err(DBError::RedisError)?;
        Ok(())
    }

    async fn delete_all_sessions_by_user(&mut self, user_id: Uuid) -> Result<(), SessionError> {
        let keys = self.find_redis_keys(user_id).await?;

        if !keys.is_empty() {
            log::debug!("Removing session, user:[{user_id}], keys: {keys:?}");
            self.client.del(keys).await.map_err(DBError::RedisError)?;
        }

        Ok(())
    }
}
