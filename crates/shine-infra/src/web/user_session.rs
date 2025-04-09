use crate::{
    db::{RedisConnectionError, RedisConnectionPool},
    web::{
        serde_session_key, ClientFingerprint, ClientFingerprintError, ErrorResponse, Problem, ProblemConfig, SessionKey,
    },
};
use axum::{extract::FromRequestParts, http::request::Parts, Extension, RequestPartsExt};
use axum_extra::extract::{cookie::Key, SignedCookieJar};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use chrono::{DateTime, Duration, Utc};
use redis::AsyncCommands;
use ring::digest;
use serde::{Deserialize, Serialize};
use shine_infra_macros::RedisJsonValue;
use std::{ops, sync::Arc};
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum UserSessionError {
    #[error("Missing session info")]
    Unauthenticated,
    #[error("Invalid session secret")]
    InvalidSecret(String),
    #[error("Invalid time to live")]
    InvalidTtl(String),
    #[error("Session expired")]
    SessionExpired,
    #[error("Fingerprint error")]
    ClientFingerprintError(#[from] ClientFingerprintError),
    #[error("Session is compromised")]
    SessionCompromised,

    #[error("Failed to get redis connection")]
    RedisPoolError(#[source] RedisConnectionError),
    #[error("Redis error")]
    RedisError(#[from] redis::RedisError),
}

impl From<UserSessionError> for Problem {
    fn from(value: UserSessionError) -> Self {
        match value {
            UserSessionError::Unauthenticated => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("unauthenticated"),
            UserSessionError::InvalidSecret(_) => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("invalidSecret"),
            UserSessionError::InvalidTtl(_) => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("invalidTtl"),
            UserSessionError::SessionExpired => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("sessionExpired"),
            UserSessionError::ClientFingerprintError(_) => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("clientFingerprintError"),
            UserSessionError::SessionCompromised => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("sessionCompromised"),

            _ => Problem::internal_error()
                .with_detail(value.to_string())
                .with_sensitive_dbg(value),
        }
    }
}

/// Information about the current user.
#[derive(Clone, Debug, Hash, Serialize, Deserialize, RedisJsonValue)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUser {
    pub user_id: Uuid,
    #[serde(with = "serde_session_key")]
    pub key: SessionKey,
    pub session_start: DateTime<Utc>,
    pub session_end: DateTime<Utc>,
    pub name: String,
    pub is_email_confirmed: bool,
    pub is_linked: bool,
    pub roles: Vec<String>,
    pub fingerprint: String,
}

/// The session cookie data.
#[derive(Clone, Debug, Hash, Serialize, Deserialize, RedisJsonValue)]
pub struct SessionCookie {
    #[serde(rename = "u")]
    pub user_id: Uuid,
    #[serde(rename = "key", with = "serde_session_key")]
    pub key: SessionKey,
    #[serde(rename = "fp")]
    pub fingerprint: String,
}

/// Extractor for the CurrentUser used to extract the user data from the request.
/// Cookie stores only the key with some minimal data, the session data is fetched from
/// the redis cache during the extraction.
pub struct CheckedCurrentUser(CurrentUser);

impl CheckedCurrentUser {
    pub fn into_user(self) -> CurrentUser {
        self.0
    }
}

impl From<CheckedCurrentUser> for CurrentUser {
    fn from(value: CheckedCurrentUser) -> Self {
        value.0
    }
}

impl ops::Deref for CheckedCurrentUser {
    type Target = CurrentUser;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl ops::DerefMut for CheckedCurrentUser {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<S> FromRequestParts<S> for CheckedCurrentUser
where
    S: Send + Sync,
{
    type Rejection = ErrorResponse<UserSessionError>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(problem_config) = parts
            .extract::<Extension<ProblemConfig>>()
            .await
            .expect("Missing ProblemConfig extension");
        let Extension(validator) = parts
            .extract::<Extension<Arc<UserSessionCacheReader>>>()
            .await
            .expect("Missing UserSessionCacheReader extension");
        let fingerprint = parts
            .extract::<ClientFingerprint>()
            .await
            .map_err(|err| ErrorResponse::new(&problem_config, UserSessionError::from(err.problem)))?;

        let jar = SignedCookieJar::from_headers(&parts.headers, validator.cookie_secret.clone());
        let session_cookie = jar
            .get(&validator.cookie_name)
            .and_then(|cookie| serde_json::from_str::<SessionCookie>(cookie.value()).ok())
            .ok_or_else(|| ErrorResponse::new(&problem_config, UserSessionError::Unauthenticated))?;

        log::debug!("Checking fingerprint: {:?}", session_cookie.user_id);
        if session_cookie.fingerprint != fingerprint.as_str() {
            return Err(ErrorResponse::new(
                &problem_config,
                UserSessionError::SessionCompromised,
            ));
        }

        log::debug!("Finding user session: {:?}", session_cookie.user_id);
        let current_user = validator
            .get_current_user(session_cookie.user_id, session_cookie.key)
            .await
            .map_err(|err| ErrorResponse::new(&problem_config, err))?;
        Ok(CheckedCurrentUser(current_user))
    }
}

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

    pub fn into_layer(self) -> Extension<Arc<Self>> {
        Extension(Arc::new(self))
    }

    /// Refresh the session data in the cache. It should be in sync with the identity service
    /// and introduce any breaking change with great care as that can break authentication in all the service.
    async fn get_current_user(&self, user_id: Uuid, session_key: SessionKey) -> Result<CurrentUser, UserSessionError> {
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
