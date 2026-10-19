use crate::{
    models::{
        messages::{StreamMessage, StreamResume, StreamSnapshot},
        CommandStreamError,
    },
    repositories::command_stream::redis::{MarkOutcome, RedisStreamEntry, RedisStreamMeta},
    services::command_stream::{
        command_stream_metrics, CommandStreamService, Fault, Observed, StreamCore, StreamEgress, StreamStores,
    },
};
use shine_game::replay::{decode_snapshot, ReplayState};
use shine_infra::db::{
    command_log::{CommandLogError, StreamId},
    redis::RedisError,
};
use std::{collections::HashSet, future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, Notify},
    time::{self, Instant},
};
use tracing::Instrument;
use uuid::Uuid;

const REDIS_READ_BATCH: i64 = 256;
/// Max entries applied per step; the rest is picked up by an immediate follow-up step.
const STEP_READ_LIMIT: usize = 1024;
const PG_READ_BATCH: i64 = 512;
/// Min time between two wake-driven steps: a busy stream sends batches, not one frame per command.
const STEP_SPACING: Duration = Duration::from_millis(2);
const RELOAD_BACKOFF_MIN: Duration = Duration::from_millis(200);
const RELOAD_BACKOFF_MAX: Duration = Duration::from_secs(10);

pub enum WorkerCommand {
    AddSubscriber(Uuid, Option<StreamResume>),
    RemoveSubscriber(Uuid),
    /// Persist what is pending and exit.
    Stop,
}

/// Control side of a running stream worker.
#[derive(Clone)]
pub struct WorkerHandle {
    pub tx: mpsc::UnboundedSender<WorkerCommand>,
    pub wake: Arc<Notify>,
}

/// Why a worker ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerExit {
    /// It was asked to stop, or its control channel closed.
    Stopped,
    /// A stored command cannot be decoded: the stream cannot be replayed.
    Unavailable,
}

pub type WorkerFuture = Pin<Box<dyn Future<Output = WorkerExit> + Send + 'static>>;

#[derive(Debug)]
enum StepError {
    /// The state must be rebuilt.
    Fault(Fault),
    /// An infrastructure error; retried on the next step.
    Failed(CommandStreamError),
}

impl From<Fault> for StepError {
    fn from(fault: Fault) -> Self {
        Self::Fault(fault)
    }
}

impl From<CommandStreamError> for StepError {
    fn from(err: CommandStreamError) -> Self {
        Self::Failed(err)
    }
}

impl From<CommandLogError> for StepError {
    #[track_caller]
    fn from(err: CommandLogError) -> Self {
        Self::Failed(err.into())
    }
}

impl From<RedisError> for StepError {
    #[track_caller]
    fn from(err: RedisError) -> Self {
        Self::Failed(err.into())
    }
}

/// Sends frames in order. A connection whose send fails gets none of its later frames (it would see
/// a gap) and leaves the core; the hub drops that connection.
async fn send_frames<S: ReplayState>(
    egress: &dyn StreamEgress,
    core: &mut StreamCore<S>,
    frames: Vec<(Uuid, StreamMessage)>,
) {
    let mut failed = HashSet::new();
    for (connection_id, message) in frames {
        if failed.contains(&connection_id) {
            continue;
        }
        if !egress.send(connection_id, message).await {
            failed.insert(connection_id);
            core.remove_subscriber(connection_id);
        }
    }
}

/// Replays one stream on this instance: the IO around a `StreamCore`.
pub struct StreamWorker<S: ReplayState> {
    stream_id: StreamId,
    core: StreamCore<S>,
    egress: Arc<dyn StreamEgress>,
    streams: CommandStreamService,
    stores: StreamStores,
    wake: Arc<Notify>,
    needs_load: bool,
    /// Earliest time of the next load attempt, and the delay after the next failure.
    next_load: Instant,
    load_backoff: Duration,
    /// When the next persist cycle is due, while commands are unpersisted.
    persist_due: Option<Instant>,
    last_step: Instant,
    /// A stored command could not be decoded.
    unavailable: bool,
}

impl<S: ReplayState> StreamWorker<S> {
    pub fn create(
        egress: Arc<dyn StreamEgress>,
        streams: CommandStreamService,
        stream_id: StreamId,
    ) -> (WorkerHandle, WorkerFuture) {
        let stores = streams.stores().clone();
        let (tx, rx) = mpsc::unbounded_channel();
        let wake = Arc::new(Notify::new());
        let worker = Self {
            core: StreamCore::new(stream_id.clone(), stores.settings.for_kind(S::KIND)),
            stream_id,
            egress,
            streams,
            stores,
            wake: wake.clone(),
            needs_load: true,
            next_load: Instant::now(),
            load_backoff: RELOAD_BACKOFF_MIN,
            persist_due: None,
            last_step: Instant::now(),
            unavailable: false,
        };
        (WorkerHandle { tx, wake }, Box::pin(worker.run(rx)))
    }

    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<WorkerCommand>) -> WorkerExit {
        tracing::debug!("[{}] Stream worker started", self.stream_id);
        let wake = self.wake.clone();
        // The first tick fires at once and performs the initial load.
        let mut ticker = time::interval(self.stores.settings.sync_interval);
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
        let exit = loop {
            let persist_at = self.persist_due.unwrap_or_else(Instant::now);
            tokio::select! {
                biased;
                command = rx.recv() => match command {
                    Some(WorkerCommand::AddSubscriber(connection_id, resume)) => {
                        match resume {
                            Some(resume) => self.core.add_subscriber_resuming(connection_id, resume),
                            None => self.core.add_subscriber(connection_id),
                        }
                        if !self.needs_load {
                            let frames = self.core.take_frames();
                            send_frames(self.egress.as_ref(), &mut self.core, frames).await;
                        }
                    }
                    Some(WorkerCommand::RemoveSubscriber(connection_id)) => self.core.remove_subscriber(connection_id),
                    Some(WorkerCommand::Stop) | None => {
                        if !self.needs_load {
                            if let Err(err) = self.persist_cycle().await {
                                tracing::warn!("[{}] Final persist failed, the orphan sweep retries it: {err:?}", self.stream_id);
                            }
                        }
                        break WorkerExit::Stopped;
                    }
                },
                _ = time::sleep_until(persist_at), if self.persist_due.is_some() => self.step(true).await,
                _ = wake.notified() => {
                    let wait = STEP_SPACING.saturating_sub(self.last_step.elapsed());
                    if !wait.is_zero() {
                        time::sleep(wait).await;
                    }
                    self.step(false).await;
                }
                _ = ticker.tick() => self.step(true).await,
            }
            if self.unavailable {
                break WorkerExit::Unavailable;
            }
        };
        tracing::debug!("[{}] Stream worker stopped", self.stream_id);
        exit
    }

    async fn step(&mut self, persist: bool) {
        let loading = self.needs_load;
        let span = tracing::debug_span!("stream step", stream.id = %self.stream_id);
        match self.try_step(persist).instrument(span).await {
            // Reset only after a full step, so a stream that reloads right after each load keeps backing off.
            Ok(()) if !self.needs_load => self.load_backoff = RELOAD_BACKOFF_MIN,
            Ok(()) => {}
            Err(StepError::Failed(err)) => {
                tracing::warn!("[{}] Stream step failed: {err:#?}", self.stream_id);
                // Only a failed load is retried with backoff; a later failure keeps the loaded state.
                if loading && self.needs_load {
                    self.schedule_load();
                }
            }
            Err(StepError::Fault(fault)) => self.begin_reload(fault).await,
        }
        self.last_step = Instant::now();
    }

    async fn try_step(&mut self, persist: bool) -> Result<(), StepError> {
        self.deliver().await?;
        if persist && !self.needs_load {
            self.persist_cycle().await?;
        }
        Ok(())
    }

    /// Loads if needed, applies the new Redis entries and sends them out.
    async fn deliver(&mut self) -> Result<(), StepError> {
        if self.needs_load {
            if Instant::now() < self.next_load {
                return Ok(());
            }
            self.load().await?;
            self.needs_load = false;
        }
        let more = self.read_redis().await?;
        let frames = self.core.take_frames();
        send_frames(self.egress.as_ref(), &mut self.core, frames).await;
        if !self.core.unpersisted().is_empty() && self.persist_due.is_none() {
            self.persist_due = Some(Instant::now() + self.persist_delay());
        }
        if more {
            self.wake.notify_one();
        }
        Ok(())
    }

    /// The configured delay plus up to the same again: the instances persisting the same commands spread
    /// their appends out instead of queueing on the stream's row lock together.
    fn persist_delay(&self) -> Duration {
        let delay = self.stores.settings.persist_delay;
        delay + Duration::from_millis(rand::random_range(0..=delay.as_millis() as u64))
    }

    /// Persists the applied commands, records the persisted version in Redis, resolves the newest
    /// checkpoint and pushes it.
    async fn persist_cycle(&mut self) -> Result<(), StepError> {
        self.persist_due = None;
        if !self.core.unpersisted().is_empty() {
            // Checked right before appending: a copy that went back in time must not be persisted.
            let tip = self.core.version();
            let (meta, entries) = self
                .stores
                .redis
                .read_after(&self.stream_id, (tip - 1).max(0), 1)
                .await?;
            if let Some(meta) = meta {
                self.observe(&meta, entries.first()).await?;
            }
        }
        if !self.core.unpersisted().is_empty() {
            let outcome = self.stores.log.append(&self.stream_id, self.core.unpersisted()).await?;
            self.core.on_append(outcome)?;
        }
        if let Some((version, command_id)) = self.core.mark_due() {
            let keep_from = self.streams.trim_floor(&self.stream_id, S::KIND).await?;
            match self
                .stores
                .redis
                .mark_persisted(&self.stream_id, self.core.generation(), version, command_id, keep_from)
                .await
            {
                Ok(MarkOutcome::Marked | MarkOutcome::Unchanged) => self.core.on_marked(version),
                Ok(MarkOutcome::Refused) => {
                    return Err(Fault::Reset {
                        generation: self.core.generation(),
                        reason: "Redis disagrees with the persisted commands",
                    }
                    .into())
                }
                Err(err) => tracing::warn!(
                    "[{}] Failed to record the persisted version in Redis: {err:#?}",
                    self.stream_id
                ),
            }
        }
        self.resolve_checkpoint().await?;
        let frames = self.core.take_checkpoint_frames();
        send_frames(self.egress.as_ref(), &mut self.core, frames).await;
        if !self.core.unpersisted().is_empty() {
            self.persist_due = Some(Instant::now() + self.persist_delay());
        }
        Ok(())
    }

    /// Races to store the newest persisted checkpoint; rebases onto the stored state when it differs.
    async fn resolve_checkpoint(&mut self) -> Result<(), StepError> {
        while let Some(checkpoint) = self.core.ready_checkpoint() {
            let outcome = self
                .stores
                .log
                .try_store_snapshot(&self.stream_id, checkpoint.version, checkpoint.hash, &checkpoint.data)
                .await?;
            if let Some(version) = self.core.on_snapshot(&checkpoint, outcome) {
                tracing::error!(
                    "[{}] State drift at version {version}: the local state differs from the authoritative snapshot",
                    self.stream_id
                );
                command_stream_metrics().drift.add(1, &[]);
                self.rebase(version).await?;
            }
        }
        Ok(())
    }

    async fn rebase(&mut self, version: i64) -> Result<(), StepError> {
        let snapshot = self
            .stores
            .log
            .latest_snapshot(&self.stream_id, version)
            .await?
            .filter(|snapshot| snapshot.version == version)
            .ok_or(Fault::Reload("the authoritative snapshot is gone"))?;
        let state = decode_snapshot::<S>(&snapshot.data)
            .map_err(|_| Fault::Reload("the authoritative snapshot is unreadable"))?;
        self.core.rebase(
            StreamSnapshot {
                version,
                hash: snapshot.hash,
                data: snapshot.data.into(),
            },
            state,
        )?;
        Ok(())
    }

    /// Rebuilds the state from the latest snapshot and the Postgres log.
    async fn load(&mut self) -> Result<(), StepError> {
        self.streams.ensure_hydrated(&self.stream_id).await?;
        let head = self
            .stores
            .log
            .head(&self.stream_id)
            .await?
            .ok_or(CommandStreamError::StreamNotFound)?;
        let snapshot = match self.stores.log.latest_snapshot(&self.stream_id, head.version).await? {
            Some(stored) => match decode_snapshot::<S>(&stored.data) {
                Ok(state) => Some((
                    StreamSnapshot {
                        version: stored.version,
                        hash: stored.hash,
                        data: stored.data.into(),
                    },
                    state,
                )),
                Err(err) => {
                    tracing::warn!(
                        "[{}] Snapshot at version {} is unreadable, replaying from the start: {err}",
                        self.stream_id,
                        stored.version
                    );
                    None
                }
            },
            None => None,
        };
        self.core.begin_load(snapshot);
        loop {
            let batch = self
                .stores
                .log
                .commands_after(&self.stream_id, self.core.version(), PG_READ_BATCH)
                .await?;
            if batch.is_empty() {
                break;
            }
            for command in batch {
                self.core.apply(command, true)?;
            }
        }
        if self.core.needs_tip() {
            // Loaded onto a snapshot with no later command: the tip's command id comes from Postgres.
            let version = self.core.version();
            if let Some(command) = self
                .stores
                .log
                .commands_after(&self.stream_id, version - 1, 1)
                .await?
                .into_iter()
                .next()
            {
                self.core.set_tip(command.version, command.command_id);
            }
        }
        let meta = self
            .stores
            .redis
            .meta(&self.stream_id)
            .await?
            .ok_or(CommandStreamError::NotReady)?;
        self.core.finish_load(&meta);
        tracing::debug!("[{}] Stream loaded at version {}", self.stream_id, self.core.version());
        Ok(())
    }

    /// Applies the Redis entries after the replica, filling holes from Postgres. Returns whether more are waiting.
    async fn read_redis(&mut self) -> Result<bool, StepError> {
        let mut applied = 0;
        while applied < STEP_READ_LIMIT {
            // Starts at the replica's own version: that entry only serves the content check.
            let tip = self.core.version();
            let (meta, entries) = self
                .stores
                .redis
                .read_after(&self.stream_id, (tip - 1).max(0), REDIS_READ_BATCH)
                .await?;
            let Some(meta) = meta else {
                // Redis lost the stream: persist what this worker holds before a new generation fences it off.
                if !self.core.unpersisted().is_empty() {
                    match self.stores.log.append(&self.stream_id, self.core.unpersisted()).await {
                        Ok(outcome) => self.core.on_append(outcome)?,
                        Err(err) => {
                            tracing::warn!("[{}] Failed to persist before rehydrating: {err:#?}", self.stream_id)
                        }
                    }
                }
                self.streams.ensure_hydrated(&self.stream_id).await?;
                return Ok(true);
            };
            self.observe(&meta, entries.first()).await?;

            let full = entries.len() as i64 == REDIS_READ_BATCH;
            let entries: Vec<_> = entries.into_iter().filter(|entry| entry.version > tip).collect();
            let Some(first) = entries.first() else {
                return Ok(false);
            };
            let expected = tip + 1;
            if first.version > expected {
                // Trimmed from Redis, so persisted: Postgres has them.
                let filled = self
                    .stores
                    .log
                    .commands_after(&self.stream_id, tip, first.version - expected)
                    .await?;
                if filled.first().map(|c| c.version) != Some(expected) {
                    return Err(Fault::Reset {
                        generation: meta.generation,
                        reason: "commands missing from Redis and Postgres",
                    }
                    .into());
                }
                applied += filled.len();
                for command in filled {
                    self.core.apply(command, true)?;
                }
                continue;
            }

            applied += entries.len();
            for entry in entries {
                self.core.apply(entry.into(), false)?;
            }
            if !full {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Checks the Redis meta and tip entry of a read, proving persistence before adopting a newer generation.
    async fn observe(&mut self, meta: &RedisStreamMeta, tip: Option<&RedisStreamEntry>) -> Result<(), StepError> {
        if let Observed::NeedsProof(generation) = self.core.observe(meta, tip)? {
            // A database error is retried by the next read: nothing was adopted or applied yet.
            let outcome = self.stores.log.append(&self.stream_id, self.core.unpersisted()).await?;
            self.core.prove_and_adopt(generation, outcome)?;
            self.core.observe(meta, tip)?;
        }
        Ok(())
    }

    async fn begin_reload(&mut self, fault: Fault) {
        match fault {
            Fault::Reload(reason) => tracing::info!("[{}] Reloading the stream: {reason}", self.stream_id),
            Fault::Reset { generation, reason } => {
                tracing::warn!(
                    "[{}] Dropping the Redis copy (generation {generation}) and reloading: {reason}",
                    self.stream_id
                );
                if let Err(err) = self.stores.redis.reset(&self.stream_id, generation).await {
                    tracing::warn!("[{}] Failed to reset the Redis copy: {err:#?}", self.stream_id);
                }
            }
            Fault::Undecodable { version } => {
                // Replay needs every version and a reload would meet the same command.
                tracing::error!(
                    "[{}] The command at version {version} cannot be decoded, the stream is unavailable",
                    self.stream_id
                );
                command_stream_metrics().decode_errors.add(1, &[]);
                self.unavailable = true;
                return;
            }
        }
        let frames = self.core.take_reset_frames();
        send_frames(self.egress.as_ref(), &mut self.core, frames).await;
        self.persist_due = None;
        self.schedule_load();
    }

    fn schedule_load(&mut self) {
        self.needs_load = true;
        self.next_load = Instant::now() + self.load_backoff;
        self.load_backoff = (self.load_backoff * 2).min(RELOAD_BACKOFF_MAX);
    }
}

#[cfg(test)]
#[path = "stream_worker_test.rs"]
mod stream_worker_test;
