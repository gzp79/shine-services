use super::scripts::{HYDRATE, IDLE_STREAMS, INGEST, MARK_PERSISTED, REQUEUE_ACTIVE, RESET, RETIRE_ACTIVE, UNLOCK};
use redis::streams::{StreamId as RedisStreamId, StreamRangeReply};
use shine_infra::db::{
    command_log::{StoredCommand, StreamId},
    redis::{RedisConnectionPool, RedisError, RedisPooledConnection},
};
use std::{sync::Arc, time::Duration};
use tracing::instrument;
use uuid::Uuid;

const HYDRATION_LOCK_MS: u64 = 5000;

#[derive(Clone, Debug)]
pub struct RedisStreamConfig {
    /// Key prefix: `RedisCommandStreamDb::PREFIX` in production, unique per test.
    pub prefix: String,
    /// Unpersisted commands per stream before writes are refused.
    pub max_unpersisted: i64,
    pub ttl_seconds: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RedisStreamMeta {
    pub generation: i64,
    pub version: i64,
    pub persisted: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedisStreamEntry {
    pub version: i64,
    pub generation: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub data: String,
}

impl From<RedisStreamEntry> for StoredCommand {
    fn from(entry: RedisStreamEntry) -> Self {
        Self {
            version: entry.version,
            command_id: entry.command_id,
            user_id: entry.user_id,
            generation: entry.generation,
            data: entry.data,
        }
    }
}

/// Outcome of recording a persisted version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkOutcome {
    Marked,
    /// Another generation, or already recorded.
    Unchanged,
    /// The copy is behind the version or holds another command there: it went back in time.
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedisIngest {
    Accepted(i64),
    /// The command id was already ingested at this version.
    Duplicate(i64),
    NotReady,
    Busy,
}

/// The tail must be the contiguous run of versions ending at `head`: the script deletes the old log
/// before it writes the new one, so it must never meet a tail it cannot write.
fn check_tail(head: i64, tail: &[StoredCommand]) -> Result<(), RedisError> {
    let valid = head >= 0
        && tail.iter().all(|c| c.version >= 1)
        && tail.windows(2).all(|w| w[1].version == w[0].version + 1)
        && tail.last().is_none_or(|last| last.version == head);
    if valid {
        Ok(())
    } else {
        Err(RedisError::RawError(redis::RedisError::from((
            redis::ErrorKind::InvalidClientConfig,
            "The hydration tail is not a contiguous run of versions ending at the head",
        ))))
    }
}

struct Keys {
    meta: String,
    log: String,
    ids: String,
    lock: String,
}

type MetaReply = (Option<i64>, Option<i64>, Option<i64>);

fn to_meta((generation, version, persisted): MetaReply) -> Option<RedisStreamMeta> {
    Some(RedisStreamMeta {
        generation: generation?,
        version: version.unwrap_or(0),
        persisted: persisted.unwrap_or(0),
    })
}

fn parse_entry(entry: &RedisStreamId) -> Option<RedisStreamEntry> {
    let (version, _) = entry.id.split_once('-')?;
    Some(RedisStreamEntry {
        version: version.parse().ok()?,
        generation: entry.get::<i64>("g")?,
        command_id: Uuid::parse_str(&entry.get::<String>("c")?).ok()?,
        user_id: Uuid::parse_str(&entry.get::<String>("u")?).ok()?,
        data: entry.get::<String>("d")?,
    })
}

struct Inner {
    pool: RedisConnectionPool,
    config: RedisStreamConfig,
    active_key: String,
    wake_channel: String,
}

/// Redis side of the command streams: ordering, dedupe, the recent window and wakes. Cheap to clone.
#[derive(Clone)]
pub struct RedisCommandStreamDb(Arc<Inner>);

impl RedisCommandStreamDb {
    pub const PREFIX: &'static str = "cs";

    pub fn new(pool: &RedisConnectionPool, mut config: RedisStreamConfig) -> Self {
        // EXPIRE with 0 deletes a key, and no room for unpersisted commands refuses every write.
        config.ttl_seconds = config.ttl_seconds.max(1);
        config.max_unpersisted = config.max_unpersisted.max(1);
        let active_key = format!("{}:active", config.prefix);
        let wake_channel = format!("{}:wake", config.prefix);
        Self(Arc::new(Inner {
            pool: pool.clone(),
            config,
            active_key,
            wake_channel,
        }))
    }

    fn keys(&self, stream_id: &StreamId) -> Keys {
        let prefix = &self.0.config.prefix;
        Keys {
            meta: format!("{prefix}:{stream_id}:meta"),
            log: format!("{prefix}:{stream_id}:log"),
            ids: format!("{prefix}:{stream_id}:ids"),
            lock: format!("{prefix}:{stream_id}:hydrating"),
        }
    }

    async fn client(&self) -> Result<RedisPooledConnection<'_>, RedisError> {
        self.0.pool.get().await.map_err(RedisError::PoolError)
    }

    /// Appends a command; the hot path, a single script.
    #[instrument(name = "redis cs ingest", level = "debug", skip_all, fields(stream.id = %stream_id))]
    pub async fn ingest(
        &self,
        stream_id: &StreamId,
        command_id: Uuid,
        user_id: Uuid,
        data: &str,
    ) -> Result<RedisIngest, RedisError> {
        let keys = self.keys(stream_id);
        let config = &self.0.config;
        let mut client = self.client().await?;
        let (status, version): (String, i64) = INGEST
            .key(&keys.meta)
            .key(&keys.log)
            .key(&keys.ids)
            .key(&self.0.active_key)
            .arg(stream_id.as_str())
            .arg(command_id.to_string())
            .arg(user_id.to_string())
            .arg(data)
            .arg(config.max_unpersisted)
            .arg(config.ttl_seconds)
            .arg(&self.0.wake_channel)
            .invoke_async(&mut *client)
            .await?;
        Ok(match status.as_str() {
            "ok" => RedisIngest::Accepted(version),
            "duplicate" => RedisIngest::Duplicate(version),
            "busy" => RedisIngest::Busy,
            _ => RedisIngest::NotReady,
        })
    }

    /// Repopulates the stream unless a same or newer generation is already there. Returns whether it wrote.
    #[instrument(name = "redis cs hydrate", level = "debug", skip_all, fields(stream.id = %stream_id))]
    pub async fn hydrate(
        &self,
        stream_id: &StreamId,
        generation: i64,
        head: i64,
        tail: &[StoredCommand],
    ) -> Result<bool, RedisError> {
        check_tail(head, tail)?;
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let mut invocation = HYDRATE.prepare_invoke();
        invocation
            .key(&keys.meta)
            .key(&keys.log)
            .key(&keys.ids)
            .key(&self.0.active_key)
            .arg(generation)
            .arg(head)
            .arg(self.0.config.ttl_seconds)
            .arg(stream_id.as_str());
        for command in tail {
            invocation
                .arg(command.version)
                .arg(command.command_id.to_string())
                .arg(command.user_id.to_string())
                .arg(command.generation)
                .arg(&command.data);
        }
        let written: i64 = invocation.invoke_async(&mut *client).await?;
        Ok(written == 1)
    }

    /// Records `version` as persisted when this copy holds `command_id` there, and trims the commands
    /// below `keep_from` (the version of the oldest stored snapshot, 1 when there is none) that are
    /// persisted. A command that is not persisted is never trimmed.
    #[instrument(name = "redis cs mark persisted", level = "debug", skip_all, fields(stream.id = %stream_id))]
    pub async fn mark_persisted(
        &self,
        stream_id: &StreamId,
        generation: i64,
        version: i64,
        command_id: Uuid,
        keep_from: i64,
    ) -> Result<MarkOutcome, RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let outcome: i64 = MARK_PERSISTED
            .key(&keys.meta)
            .key(&keys.log)
            .key(&keys.ids)
            .arg(generation)
            .arg(version)
            .arg(command_id.to_string())
            .arg(keep_from.max(1))
            .invoke_async(&mut *client)
            .await?;
        Ok(match outcome {
            1 => MarkOutcome::Marked,
            0 => MarkOutcome::Unchanged,
            _ => MarkOutcome::Refused,
        })
    }

    /// Drops the stream from Redis if it is still at `generation`.
    #[instrument(name = "redis cs reset", level = "debug", skip_all, fields(stream.id = %stream_id))]
    pub async fn reset(&self, stream_id: &StreamId, generation: i64) -> Result<bool, RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let deleted: i64 = RESET
            .key(&keys.meta)
            .key(&keys.log)
            .key(&keys.ids)
            .arg(generation)
            .invoke_async(&mut *client)
            .await?;
        Ok(deleted == 1)
    }

    pub async fn meta(&self, stream_id: &StreamId) -> Result<Option<RedisStreamMeta>, RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let reply: MetaReply = redis::cmd("HMGET")
            .arg(&keys.meta)
            .arg("generation")
            .arg("version")
            .arg("persisted")
            .query_async(&mut *client)
            .await?;
        Ok(to_meta(reply))
    }

    /// Meta and the entries above `after`, read atomically.
    pub async fn read_after(
        &self,
        stream_id: &StreamId,
        after: i64,
        limit: i64,
    ) -> Result<(Option<RedisStreamMeta>, Vec<RedisStreamEntry>), RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let (meta, reply): (MetaReply, StreamRangeReply) = redis::pipe()
            .atomic()
            .cmd("HMGET")
            .arg(&keys.meta)
            .arg("generation")
            .arg("version")
            .arg("persisted")
            .cmd("XRANGE")
            .arg(&keys.log)
            .arg(format!("({}-0", after.max(0)))
            .arg("+")
            .arg("COUNT")
            .arg(limit)
            .query_async(&mut *client)
            .await?;
        let mut entries = Vec::with_capacity(reply.ids.len());
        for raw in &reply.ids {
            match parse_entry(raw) {
                Some(entry) => entries.push(entry),
                None => tracing::error!("[{stream_id}] Unreadable command stream entry {}", raw.id),
            }
        }
        Ok((to_meta(meta), entries))
    }

    /// Returns a token when the lock was taken.
    pub async fn try_lock_hydration(&self, stream_id: &StreamId) -> Result<Option<String>, RedisError> {
        let keys = self.keys(stream_id);
        let token = Uuid::new_v4().to_string();
        let mut client = self.client().await?;
        let set: Option<String> = redis::cmd("SET")
            .arg(&keys.lock)
            .arg(&token)
            .arg("NX")
            .arg("PX")
            .arg(HYDRATION_LOCK_MS)
            .query_async(&mut *client)
            .await?;
        Ok(set.map(|_| token))
    }

    pub async fn unlock_hydration(&self, stream_id: &StreamId, token: &str) -> Result<(), RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let _: i64 = UNLOCK.key(&keys.lock).arg(token).invoke_async(&mut *client).await?;
        Ok(())
    }

    /// Streams without writes for at least `idle`, oldest first.
    pub async fn idle_streams(&self, idle: Duration, limit: usize) -> Result<Vec<StreamId>, RedisError> {
        let mut client = self.client().await?;
        let members: Vec<String> = IDLE_STREAMS
            .key(&self.0.active_key)
            .arg(idle.as_millis() as i64)
            .arg(limit)
            .invoke_async(&mut *client)
            .await?;
        Ok(members.iter().filter_map(|m| StreamId::parse(m).ok()).collect())
    }

    /// Unlists a stream with nothing unpersisted; `false` when it still has unpersisted commands.
    pub async fn retire_active(&self, stream_id: &StreamId) -> Result<bool, RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let removed: i64 = RETIRE_ACTIVE
            .key(&self.0.active_key)
            .key(&keys.meta)
            .arg(stream_id.as_str())
            .invoke_async(&mut *client)
            .await?;
        Ok(removed == 1)
    }

    /// Moves a stream that could not be retired behind the other idle streams, so it cannot starve the sweep.
    pub async fn requeue_active(&self, stream_id: &StreamId) -> Result<(), RedisError> {
        let mut client = self.client().await?;
        let _: i64 = REQUEUE_ACTIVE
            .key(&self.0.active_key)
            .arg(stream_id.as_str())
            .invoke_async(&mut *client)
            .await?;
        Ok(())
    }

    /// Calls `handler` on every ingest with the stream id; `None` after a listener reconnect (wake all).
    /// The handler runs under the listener's lock: it must not block or await.
    pub async fn listen_to_wakes<F>(&self, handler: F) -> Result<(), RedisError>
    where
        F: Fn(Option<StreamId>) + Send + Sync + 'static,
    {
        let client = self.client().await?;
        client
            .listen(&self.0.wake_channel, move |payload| {
                handler(payload.and_then(|p| StreamId::parse(p).ok()))
            })
            .await
    }
}

#[cfg(test)]
#[path = "redis_command_stream_test.rs"]
mod redis_command_stream_test;
