use crate::{
    models::CommandStreamError,
    repositories::command_stream::redis::{MarkOutcome, RedisCommandStreamDb, RedisIngest, RedisStreamMeta},
    services::command_stream::{command_stream_metrics, StreamAccess, StreamKind, StreamKindRegistry},
    settings::CommandStreamSettings,
};
use opentelemetry::KeyValue;
use shine_infra::db::command_log::{
    AppendOutcome, CommandLogError, PgCommandLogDb, StoredCommand, StreamHead, StreamId,
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::instrument;
use uuid::Uuid;

/// Idle streams persisted and retired by one sweep.
const SWEEP_BATCH: usize = 100;
/// Commands moved from Redis to Postgres in one append.
const PERSIST_BATCH: i64 = 500;
/// Longest wait for another instance that is hydrating the same stream.
const HYDRATION_WAIT: Duration = Duration::from_secs(10);
const HYDRATION_POLL: Duration = Duration::from_millis(20);

/// The stores behind the command streams.
#[derive(Clone)]
pub struct StreamStores {
    pub redis: RedisCommandStreamDb,
    pub log: PgCommandLogDb,
    pub settings: CommandStreamSettings,
}

pub struct OpenedStream {
    pub kind: Arc<dyn StreamKind>,
    pub can_write: bool,
}

#[derive(Debug)]
pub enum OpenError {
    UnknownKind,
    /// The stream exists with another kind.
    KindMismatch,
    Forbidden,
    Failed(CommandStreamError),
}

impl OpenError {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::UnknownKind => "unknown_kind",
            Self::KindMismatch => "kind_mismatch",
            Self::Forbidden => "forbidden",
            Self::Failed(_) => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IngestOutcome {
    /// Ordered at the version.
    Accepted(i64),
    /// The command id was already ordered at the version.
    Duplicate(i64),
    /// Not a valid command of the kind.
    Invalid,
    /// Redis has no copy of the stream; one is being built, the client may retry.
    NotReady,
    /// Too many unpersisted commands; the client may retry.
    Busy,
}

impl IngestOutcome {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Accepted(_) => "accepted",
            Self::Duplicate(_) => "duplicate",
            Self::Invalid => "invalid",
            Self::NotReady => "not_ready",
            Self::Busy => "busy",
        }
    }
}

/// What persisting the unpersisted commands of a stream came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistOutcome {
    /// Everything that was in Redis is in Postgres.
    Persisted,
    /// The Redis copy disagreed with Postgres and was dropped; Postgres wins.
    Dropped,
}

/// Whether a Redis copy can serve the log in Postgres: same history, and not behind it.
fn is_fresh(meta: &RedisStreamMeta, head: &StreamHead) -> bool {
    meta.generation == head.generation && meta.version >= head.version
}

/// Marks a stream as being hydrated by this instance until dropped.
struct HydrationGuard {
    streams: Arc<Mutex<HashSet<StreamId>>>,
    stream_id: StreamId,
}

impl Drop for HydrationGuard {
    fn drop(&mut self) {
        if let Ok(mut streams) = self.streams.lock() {
            streams.remove(&self.stream_id);
        }
    }
}

struct Inner {
    stores: StreamStores,
    kinds: StreamKindRegistry,
    access: Arc<dyn StreamAccess>,
    /// Streams with a background hydration in flight.
    hydrating: Arc<Mutex<HashSet<StreamId>>>,
}

/// Opens, hydrates and feeds the command streams; no connection or HTTP concerns. Cheap to clone.
#[derive(Clone)]
pub struct CommandStreamService(Arc<Inner>);

impl CommandStreamService {
    pub fn new(stores: StreamStores, kinds: StreamKindRegistry, access: Arc<dyn StreamAccess>) -> Self {
        Self(Arc::new(Inner {
            stores,
            kinds,
            access,
            hydrating: Arc::default(),
        }))
    }

    pub fn kinds(&self) -> &StreamKindRegistry {
        &self.0.kinds
    }

    pub fn settings(&self) -> &CommandStreamSettings {
        &self.0.stores.settings
    }

    pub fn stores(&self) -> &StreamStores {
        &self.0.stores
    }

    /// The version from which Redis must keep the persisted commands of a stream of `kind`: the oldest
    /// snapshot that is kept, 1 while there are not enough snapshots.
    pub async fn trim_floor(&self, stream_id: &StreamId, kind: &str) -> Result<i64, CommandStreamError> {
        let keep = self.0.stores.settings.for_kind(kind).keep_snapshots;
        Ok(self.0.stores.log.snapshot_horizon(stream_id, keep).await?.unwrap_or(1))
    }

    /// Creates the stream if it does not exist and makes sure Redis can serve it.
    #[instrument(name = "stream open", level = "debug", skip(self), fields(stream.id = %stream_id))]
    pub async fn open(&self, user_id: Uuid, stream_id: &StreamId, kind: &str) -> Result<OpenedStream, OpenError> {
        let kind = self.0.kinds.get(kind).ok_or(OpenError::UnknownKind)?;
        if !self.0.access.can_read(user_id, stream_id) {
            return Err(OpenError::Forbidden);
        }

        match self.0.stores.log.ensure_stream(stream_id, kind.kind()).await {
            Ok(_) => {}
            Err(CommandLogError::KindMismatch { .. }) => return Err(OpenError::KindMismatch),
            Err(err) => return Err(OpenError::Failed(err.into())),
        }
        self.ensure_hydrated(stream_id).await.map_err(OpenError::Failed)?;

        Ok(OpenedStream {
            can_write: self.0.access.can_write(user_id, stream_id),
            kind,
        })
    }

    /// Orders a command of an opened stream. The stored text is the canonical form of the command.
    #[instrument(name = "stream ingest", level = "debug", skip_all, fields(stream.id = %stream_id))]
    pub async fn ingest(
        &self,
        kind: &dyn StreamKind,
        stream_id: &StreamId,
        command_id: Uuid,
        user_id: Uuid,
        command: &str,
    ) -> Result<IngestOutcome, CommandStreamError> {
        let outcome = self.ingest_command(kind, stream_id, command_id, user_id, command).await;
        let label = match &outcome {
            Ok(outcome) => outcome.reason(),
            Err(_) => "error",
        };
        command_stream_metrics()
            .ingested
            .add(1, &[KeyValue::new("outcome", label)]);
        outcome
    }

    async fn ingest_command(
        &self,
        kind: &dyn StreamKind,
        stream_id: &StreamId,
        command_id: Uuid,
        user_id: Uuid,
        command: &str,
    ) -> Result<IngestOutcome, CommandStreamError> {
        if command.len() > self.0.stores.settings.max_command_bytes {
            return Ok(IngestOutcome::Invalid);
        }
        let Some(command) = kind.canonicalize(command) else {
            return Ok(IngestOutcome::Invalid);
        };

        Ok(
            match self
                .0
                .stores
                .redis
                .ingest(stream_id, command_id, user_id, &command)
                .await?
            {
                RedisIngest::Accepted(version) => IngestOutcome::Accepted(version),
                RedisIngest::Duplicate(version) => IngestOutcome::Duplicate(version),
                RedisIngest::Busy => IngestOutcome::Busy,
                RedisIngest::NotReady => {
                    self.hydrate_in_background(stream_id);
                    IngestOutcome::NotReady
                }
            },
        )
    }

    /// Starts a hydration unless one is already running here; the writer retries meanwhile.
    fn hydrate_in_background(&self, stream_id: &StreamId) {
        let Ok(mut streams) = self.0.hydrating.lock() else {
            return;
        };
        if !streams.insert(stream_id.clone()) {
            return;
        }
        drop(streams);

        let guard = HydrationGuard {
            streams: self.0.hydrating.clone(),
            stream_id: stream_id.clone(),
        };
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(err) = service.ensure_hydrated(&guard.stream_id).await {
                tracing::debug!("[{}] Background hydration failed: {err}", guard.stream_id);
            }
        });
    }

    /// Makes sure Redis holds a copy of the stream that is not behind Postgres. Concurrent callers,
    /// on any instance, converge on one hydration.
    #[instrument(name = "stream hydrate", level = "debug", skip(self), fields(stream.id = %stream_id))]
    pub async fn ensure_hydrated(&self, stream_id: &StreamId) -> Result<(), CommandStreamError> {
        let stores = &self.0.stores;
        let deadline = Instant::now() + HYDRATION_WAIT;
        loop {
            let head = stores
                .log
                .head(stream_id)
                .await?
                .ok_or(CommandStreamError::StreamNotFound)?;
            let meta = stores.redis.meta(stream_id).await?;
            if meta.as_ref().is_some_and(|meta| is_fresh(meta, &head)) {
                return Ok(());
            }

            if let Some(token) = stores.redis.try_lock_hydration(stream_id).await? {
                let hydrated = self.hydrate_locked(stream_id).await;
                if let Err(err) = stores.redis.unlock_hydration(stream_id, &token).await {
                    tracing::warn!("[{stream_id}] Releasing the hydration lock failed: {err}");
                }
                return hydrated;
            }

            if Instant::now() >= deadline {
                return Err(CommandStreamError::NotReady);
            }
            tokio::time::sleep(HYDRATION_POLL).await;
        }
    }

    /// Rebuilds the Redis copy from Postgres; the caller holds the hydration lock.
    async fn hydrate_locked(&self, stream_id: &StreamId) -> Result<(), CommandStreamError> {
        let stores = &self.0.stores;

        // Another instance may have finished while the lock was taken.
        let head = stores
            .log
            .head(stream_id)
            .await?
            .ok_or(CommandStreamError::StreamNotFound)?;
        let meta = stores.redis.meta(stream_id).await?;
        if let Some(meta) = &meta {
            if is_fresh(meta, &head) {
                return Ok(());
            }
            // A copy of another history is dropped, the hydration never overwrites a newer one.
            stores.redis.reset(stream_id, meta.generation).await?;
        }

        let head = stores.log.begin_generation(stream_id).await?;
        let settings = stores.settings.for_kind(&head.kind);
        let horizon = stores.log.snapshot_horizon(stream_id, settings.keep_snapshots).await?;
        let limit = settings.snapshot_every * settings.keep_snapshots + settings.snapshot_every + 1;
        let tail: Vec<StoredCommand> = stores
            .log
            .last_commands(stream_id, limit)
            .await?
            .into_iter()
            .filter(|command| command.version <= head.version && horizon.is_none_or(|h| command.version >= h))
            .collect();

        stores
            .redis
            .hydrate(stream_id, head.generation, head.version, &tail)
            .await?;
        Ok(())
    }

    /// Moves the unpersisted commands of a stream into Postgres. A Redis copy that disagrees with
    /// Postgres is dropped: the log wins.
    #[instrument(name = "stream persist", level = "debug", skip(self), fields(stream.id = %stream_id))]
    pub async fn persist_tail(&self, stream_id: &StreamId) -> Result<PersistOutcome, CommandStreamError> {
        let stores = &self.0.stores;
        let Some(meta) = stores.redis.meta(stream_id).await? else {
            return Ok(PersistOutcome::Persisted);
        };
        let Some(head) = stores.log.head(stream_id).await? else {
            return self.drop_copy(stream_id, &meta, "unknown to Postgres").await;
        };
        if meta.generation != head.generation {
            return self.drop_copy(stream_id, &meta, "another generation").await;
        }

        let mut last = None;
        let mut after = meta.persisted;
        while after < meta.version {
            let (_, entries) = stores.redis.read_after(stream_id, after, PERSIST_BATCH).await?;
            let Some(batch_last) = entries.last().map(|entry| (entry.version, entry.command_id)) else {
                break;
            };
            let commands: Vec<StoredCommand> = entries.into_iter().map(StoredCommand::from).collect();
            match stores.log.append(stream_id, &commands).await? {
                AppendOutcome::Appended { .. } | AppendOutcome::UpToDate { .. } => {}
                outcome => return self.drop_copy(stream_id, &meta, &format!("append: {outcome:?}")).await,
            }
            after = batch_last.0;
            last = Some(batch_last);
        }

        if let Some((version, command_id)) = last {
            let keep_from = self.trim_floor(stream_id, &head.kind).await?;
            let marked = stores
                .redis
                .mark_persisted(stream_id, meta.generation, version, command_id, keep_from)
                .await?;
            if marked == MarkOutcome::Refused {
                return self.drop_copy(stream_id, &meta, "mark refused").await;
            }
        }
        Ok(PersistOutcome::Persisted)
    }

    async fn drop_copy(
        &self,
        stream_id: &StreamId,
        meta: &RedisStreamMeta,
        reason: &str,
    ) -> Result<PersistOutcome, CommandStreamError> {
        tracing::warn!("[{stream_id}] Dropping the Redis copy of the stream ({reason})");
        self.0.stores.redis.reset(stream_id, meta.generation).await?;
        Ok(PersistOutcome::Dropped)
    }

    /// Persists and unlists the streams nobody wrote to for a while. Streams that cannot be retired
    /// are queued behind the others and retried in a later sweep.
    pub async fn sweep_orphans(&self) {
        let stores = &self.0.stores;
        let idle = match stores
            .redis
            .idle_streams(stores.settings.orphan_idle, SWEEP_BATCH)
            .await
        {
            Ok(idle) => idle,
            Err(err) => {
                tracing::warn!("Listing the idle command streams failed: {err}");
                return;
            }
        };

        for stream_id in idle {
            let retired = match self.persist_tail(&stream_id).await {
                Ok(_) => stores.redis.retire_active(&stream_id).await,
                Err(err) => {
                    tracing::warn!("[{stream_id}] Persisting the idle stream failed: {err}");
                    Ok(false)
                }
            };
            if !matches!(retired, Ok(true)) {
                if let Err(err) = stores.redis.requeue_active(&stream_id).await {
                    tracing::warn!("[{stream_id}] Requeueing the idle stream failed: {err}");
                }
            }
        }
    }

    /// Calls `handler` for every ingest with the stream id; `None` after a reconnect of the listener.
    pub async fn listen_to_wakes<F>(&self, handler: F) -> Result<(), CommandStreamError>
    where
        F: Fn(Option<StreamId>) + Send + Sync + 'static,
    {
        Ok(self.0.stores.redis.listen_to_wakes(handler).await?)
    }
}

#[cfg(test)]
#[path = "command_stream_test.rs"]
mod command_stream_test;
