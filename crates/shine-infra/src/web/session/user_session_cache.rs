use crate::{
    db::RedisConnectionPool,
    web::{session::CurrentUser, session::SessionKey},
};
use axum::Extension;
use axum_extra::extract::cookie::Key;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use chrono::{DateTime, Duration, Utc};
use redis::AsyncCommands;
use ring::digest;
use serde::{Deserialize, Serialize};
use shine_infra_macros::RedisJsonValue;
use std::sync::Arc;
use uuid::Uuid;

use super::UserSessionError;

/// Handle the user data query in the redis cache.
pub struct UserSessionCacheReader {
    cookie_name: String,
    cookie_secret: Key,
    key_prefix: String,
    ttl_session: i64,
    redis: RedisConnectionPool,
}

impl UserSessionCacheReader {
    pub fn new(
        name_suffix: Option<&str>,
        cookie_secret: &str,
        key_prefix: &str,
        ttl_session: u64,
        redis: RedisConnectionPool,
    ) -> Result<Self, UserSessionError> {
        let name_suffix = name_suffix.unwrap_or_default();
        let cookie_secret = {
            let key = B64
                .decode(cookie_secret)
                .map_err(|err| UserSessionError::InvalidSecret(format!("{err}")))?;
            Key::try_from(&key[..]).map_err(|err| UserSessionError::InvalidSecret(format!("{err}")))?
        };
        let ttl_session = ttl_session
            .try_into()
            .map_err(|err| UserSessionError::InvalidTtl(format!("{err}")))?;

        Ok(Self {
            cookie_name: format!("sid{}", name_suffix),
            cookie_secret,
            key_prefix: key_prefix.to_string(),
            ttl_session,
            redis,
        })
    }

    pub fn cookie_name(&self) -> &str {
        &self.cookie_name
    }

    pub fn cookie_secret(&self) -> &Key {
        &self.cookie_secret
    }

    pub fn into_layer(self) -> Extension<Arc<Self>> {
        Extension(Arc::new(self))
    }

    /// Refresh the session data in the cache. It should be in sync with the identity service
    /// and introduce any breaking change with great care as that can break authentication in all the service.
    pub async fn get_current_user(
        &self,
        user_id: Uuid,
        session_key: SessionKey,
    ) -> Result<CurrentUser, UserSessionError> {
        #[derive(Serialize, Deserialize, Debug, RedisJsonValue)]
        #[serde(rename_all = "camelCase")]
        struct SessionSentinel {
            pub created_at: DateTime<Utc>,
            pub fingerprint: String,
        }

        #[derive(Serialize, Deserialize, Debug, RedisJsonValue)]
        #[serde(rename_all = "camelCase")]
        struct SessionData {
            pub name: String,
            pub is_email_confirmed: bool,
            pub is_linked: bool,
            pub roles: Vec<String>,
        }

        let (sentinel_key, key) = {
            let key_hash = digest::digest(&digest::SHA256, session_key.as_bytes());
            let key_hash = hex::encode(key_hash);

            let prefix = format!("{}session:{}:{}", self.key_prefix, user_id.as_simple(), key_hash);
            let sentinel_key = format!("{prefix}:sentinel");
            let key = format!("{prefix}:data");
            (sentinel_key, key)
        };

        let mut client = self.redis.get().await.map_err(UserSessionError::RedisPoolError)?;

        // query sentinel
        let sentinel: SessionSentinel = match client.get(&sentinel_key).await.map_err(UserSessionError::RedisError)? {
            Some(sentinel) => sentinel,
            _ => return Err(UserSessionError::SessionExpired),
        };

        // find user data. In a very unlikely case data could have been just deleted.
        let data: SessionData = match client.get(&key).await.map_err(UserSessionError::RedisError)? {
            Some(data) => data,
            _ => return Err(UserSessionError::SessionExpired),
        };

        // extend session expiration
        //todo: should we have a maximum session lifetime based on the creation time?
        let _: () = redis::pipe()
            .expire(&sentinel_key, self.ttl_session)
            .expire(&key, self.ttl_session)
            .query_async(&mut *client)
            .await
            .map_err(UserSessionError::RedisError)?;

        Ok(CurrentUser {
            user_id,
            key: session_key,
            session_start: sentinel.created_at,
            session_end: Utc::now() + Duration::seconds(self.ttl_session),
            name: data.name,
            is_email_confirmed: data.is_email_confirmed,
            is_linked: data.is_linked,
            roles: data.roles,
            fingerprint: sentinel.fingerprint.clone(),
        })
    }
}
