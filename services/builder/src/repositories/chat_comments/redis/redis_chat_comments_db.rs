use crate::{
    models::ChatError,
    repositories::chat_comments::{
        redis::RedisChatCommentsBuildError,
        ChatCommentDb,
        ChatCommentDbContext,
    },
};
use shine_infra::db::redis::{RedisConnectionPool, RedisError, RedisPooledConnection};

pub struct RedisChatCommentDbContext<'c> {
    pub(in crate::repositories::chat_comments::redis) client: RedisPooledConnection<'c>,
    pub(in crate::repositories::chat_comments::redis) max_messages: usize,
    pub(in crate::repositories::chat_comments::redis) ttl_seconds: u64,
}

impl<'c> ChatCommentDbContext<'c> for RedisChatCommentDbContext<'c> {}

#[derive(Clone)]
pub struct RedisChatCommentsDb {
    client: RedisConnectionPool,
    max_messages: usize,
    ttl_seconds: u64,
}

impl RedisChatCommentsDb {
    pub async fn new(
        redis: &RedisConnectionPool,
        max_messages: usize,
        ttl_seconds: u64,
    ) -> Result<Self, RedisChatCommentsBuildError> {
        if max_messages == 0 {
            return Err(RedisChatCommentsBuildError::InvalidConfig("Chat stream max_messages must be > 0"));
        }

        if ttl_seconds == 0 {
            return Err(RedisChatCommentsBuildError::InvalidConfig("Chat stream ttl_seconds must be > 0"));
        }

        let _client = redis.get().await.map_err(RedisError::PoolError)?;

        Ok(Self {
            client: redis.clone(),
            max_messages,
            ttl_seconds,
        })
    }
}

impl ChatCommentDb for RedisChatCommentsDb {
    async fn create_context(&self) -> Result<impl ChatCommentDbContext<'_>, ChatError> {
        let client = self
            .client
            .get()
            .await
            .map_err(RedisError::PoolError)
            .map_err(ChatError::internal)?;

        Ok(RedisChatCommentDbContext {
            client,
            max_messages: self.max_messages,
            ttl_seconds: self.ttl_seconds,
        })
    }
}
