use crate::{
    models::CommandInboxError,
    repositories::command_inbox::{
        redis::{inbox, wake},
        CommandInboxDb, InboxBatch, InboxId, InboxWake, StreamId,
    },
};
use shine_infra::db::redis::{RedisConnectionPool, RedisError, RedisPooledConnection};
use uuid::Uuid;

#[derive(Clone)]
pub struct RedisCommandInboxDb {
    pool: RedisConnectionPool,
    ttl_seconds: u64,
}

impl RedisCommandInboxDb {
    pub async fn new(redis: &RedisConnectionPool, ttl_seconds: u64) -> Result<Self, CommandInboxError> {
        if ttl_seconds == 0 {
            return Err(CommandInboxError::InvalidConfig(
                "Command inbox ttl_seconds must be > 0",
            ));
        }
        let _client = redis.get().await.map_err(RedisError::PoolError)?;
        Ok(Self {
            pool: redis.clone(),
            ttl_seconds,
        })
    }

    pub fn stream_key(stream_id: &StreamId) -> String {
        format!("cmd-inbox:{stream_id}")
    }

    pub fn ttl_seconds(&self) -> u64 {
        self.ttl_seconds
    }

    pub async fn client(&self) -> Result<RedisPooledConnection<'_>, CommandInboxError> {
        Ok(self.pool.get().await.map_err(RedisError::PoolError)?)
    }
}

impl CommandInboxDb for RedisCommandInboxDb {
    async fn push(
        &self,
        stream_id: &StreamId,
        command_id: Uuid,
        user_id: Uuid,
        data: &str,
    ) -> Result<InboxId, CommandInboxError> {
        inbox::push(self, stream_id, command_id, user_id, data).await
    }

    async fn poll(&self, stream_id: &StreamId, after: &InboxId, limit: usize) -> Result<InboxBatch, CommandInboxError> {
        inbox::poll(self, stream_id, after, limit).await
    }

    async fn trim(&self, stream_id: &StreamId, up_to: &InboxId) -> Result<usize, CommandInboxError> {
        inbox::trim(self, stream_id, up_to).await
    }

    async fn notify_persisted(&self, stream_id: &StreamId, version: i64) -> Result<(), CommandInboxError> {
        wake::notify_persisted(self, stream_id, version).await
    }

    async fn listen<F>(&self, handler: F) -> Result<(), CommandInboxError>
    where
        F: Fn(InboxWake) + Send + Sync + 'static,
    {
        wake::listen(self, handler).await
    }

    async fn unlisten(&self) -> Result<(), CommandInboxError> {
        wake::unlisten(self).await
    }
}
