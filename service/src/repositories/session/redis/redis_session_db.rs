use chrono::Duration;
use shine_service::service::{RedisConnectionPool, RedisPooledConnection};

use crate::repositories::{
    session::session_db::{SessionDb, SessionDbContext},
    DBError, SessionBuildError, SessionError,
};

pub struct RedisSessionTransaction<'a> {
    pub transaction: RedisPooledConnection<'a>,
    pub key_prefix: &'a str,
    pub ttl_session: i64,
}

pub struct RedisSessionDbContext<'c> {
    client: &'c RedisConnectionPool,
    key_prefix: &'c str,
    ttl_session: i64,
}

impl<'c> SessionDbContext<'c> for RedisSessionDbContext<'c> {
    type Transaction<'a> = RedisSessionTransaction<'a> where 'c: 'a;

    async fn begin_transaction<'a>(&'a mut self) -> Result<Self::Transaction<'a>, SessionError> {
        let client = self.client.get().await.map_err(DBError::RedisPoolError)?;

        Ok(RedisSessionTransaction {
            transaction: client,
            key_prefix: self.key_prefix,
            ttl_session: self.ttl_session,
        })
    }
}

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
    type Context<'c> = RedisSessionDbContext<'c>;

    async fn create_context(&self) -> Result<Self::Context<'_>, SessionError> {
        Ok(RedisSessionDbContext {
            client: &self.client,
            key_prefix: &self.key_prefix,
            ttl_session: self.ttl_session,
        })
    }
}
