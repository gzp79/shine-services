use crate::{
    models::CommandStreamError,
    repositories::{
        command_inbox::{CommandInboxDb, InboxId},
        command_log::{CommandLogDb, StoredCommand, StoredSnapshot},
    },
    services::command_stream::{stream_state::to_envelope, CommandStreamConfig},
};
use shine_game::replica::{decode_snapshot, encode_snapshot, Checkpoint, ReplayState, Replica, StreamId};
use std::sync::Arc;
use tokio::{
    sync::mpsc,
    task::{self, JoinHandle},
};

const CHANNEL_CAPACITY: usize = 1;

/// One optimistic batch submitted by the stream actor. Only one batch per stream may be in flight.
#[derive(Debug)]
pub struct PersistJob {
    pub generation: u64,
    pub baseline_version: i64,
    pub baseline_chain_hash: u64,
    pub commands: Arc<[StoredCommand]>,
}

enum PersistenceRequest {
    Load { generation: u64, reason: String },
    Persist(PersistJob),
}

/// The canonical result of a persistence attempt. Client delivery remains the stream actor's job.
#[derive(Debug)]
pub enum PersistResult {
    Advanced {
        generation: u64,
        canonical: Arc<[StoredCommand]>,
        snapshot: Option<StoredSnapshot>,
        checkpoint: Option<Checkpoint>,
    },
    Reloaded {
        generation: u64,
        snapshot: Option<StoredSnapshot>,
        tail: Arc<[StoredCommand]>,
        reason: String,
    },
    Failed {
        generation: u64,
        error: CommandStreamError,
    },
}

#[derive(Clone)]
pub struct PersistenceHandle {
    requests: mpsc::Sender<PersistenceRequest>,
}

impl PersistenceHandle {
    pub async fn load(&self, generation: u64, reason: impl Into<String>) -> bool {
        self.requests
            .send(PersistenceRequest::Load {
                generation,
                reason: reason.into(),
            })
            .await
            .is_ok()
    }

    pub async fn persist(&self, job: PersistJob) -> Result<(), PersistJob> {
        self.requests
            .send(PersistenceRequest::Persist(job))
            .await
            .map_err(|error| match error.0 {
                PersistenceRequest::Persist(job) => job,
                PersistenceRequest::Load { .. } => unreachable!(),
            })
    }
}

pub struct PersistenceResults {
    results: mpsc::Receiver<PersistResult>,
}

impl PersistenceResults {
    pub async fn recv(&mut self) -> Option<PersistResult> {
        self.results.recv().await
    }

    pub fn try_recv(&mut self) -> Result<PersistResult, mpsc::error::TryRecvError> {
        self.results.try_recv()
    }
}

pub struct StartedPersistence {
    pub handle: PersistenceHandle,
    pub results: PersistenceResults,
    pub task: JoinHandle<()>,
    pub snapshot: Option<StoredSnapshot>,
    pub tail: Vec<StoredCommand>,
}

/// Serializes durable append, canonical catch-up, replay and snapshot storage for one stream.
pub struct PersistenceWorker<S, I, L>
where
    S: ReplayState,
    I: CommandInboxDb,
    L: CommandLogDb,
{
    stream_id: StreamId,
    inbox: I,
    log: L,
    config: CommandStreamConfig,
    replica: Replica<S>,
}

impl<S, I, L> PersistenceWorker<S, I, L>
where
    S: ReplayState,
    I: CommandInboxDb + 'static,
    L: CommandLogDb + 'static,
{
    pub fn new(stream_id: StreamId, inbox: I, log: L, config: CommandStreamConfig) -> Self {
        Self {
            stream_id,
            inbox,
            log,
            config,
            replica: Replica::new(),
        }
    }

    pub fn version(&self) -> i64 {
        self.replica.version()
    }

    pub fn chain_hash(&self) -> u64 {
        self.replica.chain_hash()
    }

    pub fn spawn(mut self) -> (PersistenceHandle, PersistenceResults, JoinHandle<()>) {
        let (requests, mut request_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (result_tx, results) = mpsc::channel(CHANNEL_CAPACITY);
        let task = task::spawn(async move {
            while let Some(request) = request_rx.recv().await {
                let result = match request {
                    PersistenceRequest::Load { generation, reason } => self.reload(generation, reason).await,
                    PersistenceRequest::Persist(job) => self.persist(job).await,
                };
                if result_tx.send(result).await.is_err() {
                    break;
                }
            }
        });
        (PersistenceHandle { requests }, PersistenceResults { results }, task)
    }

    /// Loads or creates the stream before the actor starts, then starts the persistence loop with the
    /// rebuilt replica as its private mutable state.
    pub async fn start(mut self) -> Result<StartedPersistence, CommandStreamError> {
        let (snapshot, tail, replica) = self.load_canonical().await?;
        self.replica = replica;
        let (handle, results, task) = self.spawn();
        Ok(StartedPersistence {
            handle,
            results,
            task,
            snapshot,
            tail,
        })
    }

    async fn persist(&mut self, job: PersistJob) -> PersistResult {
        let generation = job.generation;
        if job.baseline_version != self.replica.version() || job.baseline_chain_hash != self.replica.chain_hash() {
            return self
                .reload(
                    generation,
                    format!(
                        "persistence baseline {} does not match canonical version {}",
                        job.baseline_version,
                        self.replica.version()
                    ),
                )
                .await;
        }
        let Some(last) = job.commands.last() else {
            return PersistResult::Advanced {
                generation,
                canonical: Arc::from([]),
                snapshot: None,
                checkpoint: None,
            };
        };
        let proposed = job.commands.clone();

        let canonical = match self
            .log
            .append(&self.stream_id, &job.commands, job.baseline_chain_hash)
            .await
        {
            Ok(head) if head.version == last.version && head.chain_hash == last.chain_hash => job.commands,
            Ok(_) | Err(CommandStreamError::LogDiverged { .. }) => match self.catch_up().await {
                Ok(commands) if !commands.is_empty() => commands.into(),
                Ok(_) => {
                    return self
                        .reload(
                            generation,
                            "the command log advanced without a readable continuation".to_string(),
                        )
                        .await;
                }
                Err(error) => return PersistResult::Failed { generation, error },
            },
            Err(error) => return PersistResult::Failed { generation, error },
        };
        let shared = canonical.len().min(proposed.len());
        if canonical[..shared] != proposed[..shared] {
            return self
                .reload(
                    generation,
                    "canonical commands differ from the optimistic history".to_string(),
                )
                .await;
        }

        match self.advance(&canonical).await {
            Ok((snapshot, checkpoint)) => {
                self.finish(&canonical).await;
                PersistResult::Advanced {
                    generation,
                    canonical,
                    snapshot,
                    checkpoint,
                }
            }
            Err(AdvanceError::Reload(reason)) => self.reload(generation, reason).await,
            Err(AdvanceError::Failed(error)) => PersistResult::Failed { generation, error },
        }
    }

    async fn reload(&mut self, generation: u64, reason: String) -> PersistResult {
        match self.load_canonical().await {
            Ok((snapshot, tail, replica)) => {
                self.replica = replica;
                PersistResult::Reloaded {
                    generation,
                    snapshot,
                    tail: tail.into(),
                    reason,
                }
            }
            Err(error) => PersistResult::Failed { generation, error },
        }
    }

    async fn load_canonical(
        &self,
    ) -> Result<(Option<StoredSnapshot>, Vec<StoredCommand>, Replica<S>), CommandStreamError> {
        let head = self.log.ensure_stream(&self.stream_id).await?;
        let snapshot = self.log.latest_snapshot(&self.stream_id, head.version).await?;
        let (mut replica, mut after) = match &snapshot {
            Some(snapshot) => {
                let state = decode_snapshot::<S>(&snapshot.data)
                    .map_err(|error| CommandStreamError::Corrupt(format!("snapshot {}: {error}", snapshot.version)))?;
                (
                    Replica::from_snapshot(snapshot.version, state, snapshot.chain_hash),
                    snapshot.version,
                )
            }
            None => (Replica::new(), 0),
        };
        let mut tail = Vec::new();
        loop {
            let page = self
                .log
                .commands_after(&self.stream_id, after, self.config.page_size)
                .await?;
            let count = page.len() as i64;
            for command in page {
                let envelope = to_envelope::<S>(&command)?;
                replica
                    .apply_next(&envelope)
                    .map_err(|error| CommandStreamError::Corrupt(format!("command {}: {error}", command.version)))?;
                after = command.version;
                tail.push(command);
            }
            if count < self.config.page_size {
                return Ok((snapshot, tail, replica));
            }
        }
    }

    async fn catch_up(&self) -> Result<Vec<StoredCommand>, CommandStreamError> {
        let mut after = self.replica.version();
        let mut commands = Vec::new();
        loop {
            let page = self
                .log
                .commands_after(&self.stream_id, after, self.config.page_size)
                .await?;
            let count = page.len() as i64;
            if let Some(last) = page.last() {
                after = last.version;
            }
            commands.extend(page);
            if count < self.config.page_size {
                return Ok(commands);
            }
        }
    }

    async fn advance(
        &mut self,
        commands: &[StoredCommand],
    ) -> Result<(Option<StoredSnapshot>, Option<Checkpoint>), AdvanceError> {
        let mut snapshot = None;
        for command in commands {
            let envelope = to_envelope::<S>(command).map_err(AdvanceError::Failed)?;
            self.replica.apply_next(&envelope).map_err(|error| {
                AdvanceError::Reload(format!(
                    "canonical command {} cannot be replayed: {error}",
                    command.version
                ))
            })?;
            if self.config.snapshot_every > 0 && command.version % self.config.snapshot_every == 0 {
                let data = encode_snapshot(self.replica.state()).map_err(|error| {
                    AdvanceError::Failed(CommandStreamError::Corrupt(format!(
                        "snapshot {}: {error}",
                        command.version
                    )))
                })?;
                snapshot = Some(StoredSnapshot {
                    version: command.version,
                    hash: self.replica.hash(),
                    chain_hash: self.replica.chain_hash(),
                    data,
                });
            }
        }

        let (snapshot, checkpoint) = if let Some(snapshot) = snapshot {
            match self.log.try_store_snapshot(&self.stream_id, &snapshot).await {
                Ok(()) => {
                    if let Err(error) = self.log.prune(&self.stream_id, self.config.keep_snapshots).await {
                        tracing::warn!("[{}] Failed to prune the log: {error:#?}", self.stream_id);
                    }
                    let checkpoint = Checkpoint {
                        version: snapshot.version,
                        chain_hash: snapshot.chain_hash,
                        state_hash: snapshot.hash,
                    };
                    (Some(snapshot), Some(checkpoint))
                }
                Err(CommandStreamError::SnapshotDiverged) => {
                    return Err(AdvanceError::Reload(format!(
                        "snapshot {} differs from the stored snapshot",
                        snapshot.version
                    )));
                }
                Err(error) => {
                    tracing::warn!(
                        "[{}] Failed to store the snapshot of version {}: {error:#?}",
                        self.stream_id,
                        snapshot.version
                    );
                    (None, None)
                }
            }
        } else {
            (None, None)
        };
        Ok((snapshot, checkpoint))
    }

    async fn finish(&self, commands: &[StoredCommand]) {
        let trim_through = commands
            .iter()
            .filter_map(|command| command.inbox_id.as_deref().and_then(InboxId::parse))
            .max();
        if let Some(trim_through) = trim_through {
            if let Err(error) = self.inbox.trim(&self.stream_id, &trim_through).await {
                tracing::warn!("[{}] Failed to trim the inbox: {error:#?}", self.stream_id);
            }
        }
        if let Err(error) = self
            .inbox
            .notify_persisted(&self.stream_id, self.replica.version())
            .await
        {
            tracing::warn!(
                "[{}] Failed to announce the persisted version: {error:#?}",
                self.stream_id
            );
        }
    }
}

enum AdvanceError {
    Reload(String),
    Failed(CommandStreamError),
}

#[cfg(test)]
#[path = "persistence_worker_test.rs"]
mod persistence_worker_test;
