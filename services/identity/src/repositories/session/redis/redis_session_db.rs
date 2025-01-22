use chrono::Duration;
use shine_core::db::{DBError, RedisConnectionPool, RedisPooledConnection};

use crate::repositories::session::{SessionBuildError, SessionDb, SessionDbContext, SessionError};

pub struct RedisSessionDbContext<'c> {
    pub(in crate::repositories::session::redis) client: RedisPooledConnection<'c>,
    pub(in crate::repositories::session::redis) key_prefix: &'c str,
    pub(in crate::repositories::session::redis) ttl_session: i64,
}

impl<'c> SessionDbContext<'c> for RedisSessionDbContext<'c> {}

#[derive(Clone)]
pub struct RedisSessionDb {
    client: RedisConnectionPool,
    key_prefix: String,
    ttl_session: i64,
}

impl RedisSessionDb {
    pub async fn new(
        redis: &RedisConnectionPool,
        key_prefix: String,
        ttl_session: Duration,
    ) -> Result<Self, SessionBuildError> {
        let _client = redis.get().await.map_err(DBError::RedisPoolError)?;
        //todo: check/update permissions to allow update only from identity service

        Ok(Self {
            client: redis.clone(),
            key_prefix,
            ttl_session: ttl_session.num_seconds(),
        })
    }
}

impl SessionDb for RedisSessionDb {
    async fn create_context(&self) -> Result<impl SessionDbContext<'_>, SessionError> {
        let client = self.client.get().await.map_err(DBError::RedisPoolError)?;

        Ok(RedisSessionDbContext {
            client,
            key_prefix: &self.key_prefix,
            ttl_session: self.ttl_session,
        })
    }
}
