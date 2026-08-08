use crate::{
    models::ChatError,
    repositories::chat_comments::{redis::RedisChatCommentsBuildError, ChatCommentDb, ChatCommentDbContext},
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
            return Err(RedisChatCommentsBuildError::InvalidConfig(
                "Chat stream max_messages must be > 0",
            ));
        }

        if ttl_seconds == 0 {
            return Err(RedisChatCommentsBuildError::InvalidConfig(
                "Chat stream ttl_seconds must be > 0",
            ));
        }

        let _client = redis.get().await.map_err(RedisError::PoolError)?;

        Ok(Self {
            client: redis.clone(),
            max_messages,
            ttl_seconds,
        })
    }

    fn to_stream_key(room_id: &str) -> String {
        format!("chat:{room_id}")
    }

    /// Listens for appends to a room's stream. `append_comment` publishes the new stream id on the
    /// room's stream key; every notification is a plain wake — the payload is ignored because the
    /// consumer reads from its own cursor, so it only needs to know that something changed. A `None`
    /// payload (listener reconnect) also fires: appends published during the outage were lost to
    /// pub/sub, so the wake lets the consumer pick them up ahead of its next periodic pass.
    pub async fn listen_to_room_changes<F>(&self, room_id: &str, handler: F) -> Result<(), ChatError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let stream_key = Self::to_stream_key(room_id);
        let client = self
            .client
            .get()
            .await
            .map_err(RedisError::PoolError)
            .map_err(ChatError::internal)?;
        client
            .listen(&stream_key, move |_payload| handler())
            .await
            .map_err(ChatError::internal)?;
        Ok(())
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
