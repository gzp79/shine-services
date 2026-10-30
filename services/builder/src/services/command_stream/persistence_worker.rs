use crate::{
    models::CommandStreamError,
    repositories::{
        command_inbox::CommandInboxDb,
        command_log::{CommandLogDb, StoredCommand, StoredSnapshot},
    },
    services::command_stream::{
        stream_log_service::{AppendOutcome, SnapshotOutcome, StreamLogService},
        CommandStreamConfig,
    },
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
    service: StreamLogService<I, L>,
    replica: Replica<S>,
}

impl<S, I, L> PersistenceWorker<S, I, L>
where
    S: ReplayState,
    I: CommandInboxDb + 'static,
    L: CommandLogDb + 'static,
{
    pub fn new(stream_id: StreamId, inbox: I, log: L, config: CommandStreamConfig) -> Self {
        Self::with_service(stream_id, StreamLogService::new(inbox, log, config))
    }

    pub fn with_service(stream_id: StreamId, service: StreamLogService<I, L>) -> Self {
        Self {
            stream_id,
            service,
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
        if job.commands.is_empty() {
            return PersistResult::Advanced {
                generation,
                canonical: Arc::from([]),
                snapshot: None,
                checkpoint: None,
            };
        }
        let proposed = job.commands.clone();

        let canonical = match self
            .service
            .append(
                &self.stream_id,
                &job.commands,
                job.baseline_version,
                job.baseline_chain_hash,
            )
            .await
        {
            Ok(AppendOutcome::Stored) => job.commands,
            Ok(AppendOutcome::Behind { canonical }) if !canonical.is_empty() => canonical.into(),
            Ok(AppendOutcome::Behind { .. }) => {
                return self
                    .reload(
                        generation,
                        "the command log advanced without a readable continuation".to_string(),
                    )
                    .await;
            }
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
        let stored = self.service.load(&self.stream_id).await?;
        let replica = Self::rebuild(stored.snapshot.as_ref(), &stored.tail)?;
        Ok((stored.snapshot, stored.tail, replica))
    }

    /// Rebuilds the replica from the given snapshot and commands.
    fn rebuild(
        snapshot: Option<&StoredSnapshot>,
        commands: &[StoredCommand],
    ) -> Result<Replica<S>, CommandStreamError> {
        let mut replica = match snapshot {
            Some(snapshot) => {
                let state = decode_snapshot::<S>(&snapshot.data)
                    .map_err(|error| CommandStreamError::Corrupt(format!("snapshot {}: {error}", snapshot.version)))?;
                Replica::from_snapshot(snapshot.version, state, snapshot.chain_hash)
            }
            None => Replica::new(),
        };
        for command in commands {
            let envelope = command.to_envelope::<S>()?;
            replica
                .apply_next(&envelope)
                .map_err(|error| CommandStreamError::Corrupt(format!("command {}: {error}", command.version)))?;
        }
        Ok(replica)
    }

    /// Replays the commands onto the replica, in place. A command that does not continue the history
    /// means this instance and the log do not agree: the replica has to be built again from the log.
    fn apply(&mut self, commands: &[StoredCommand]) -> Result<(), AdvanceError> {
        for command in commands {
            let envelope = command.to_envelope::<S>().map_err(AdvanceError::Failed)?;
            self.replica.apply_next(&envelope).map_err(|error| {
                AdvanceError::Reload(format!(
                    "canonical command {} cannot be replayed: {error}",
                    command.version
                ))
            })?;
        }
        Ok(())
    }

    /// Takes the replica's state as a snapshot row at the given version.
    fn snapshot_at(&self, version: i64) -> Result<StoredSnapshot, CommandStreamError> {
        let data = encode_snapshot(self.replica.state())
            .map_err(|error| CommandStreamError::Corrupt(format!("snapshot {version}: {error}")))?;
        Ok(StoredSnapshot {
            version,
            hash: self.replica.hash(),
            chain_hash: self.replica.chain_hash(),
            data,
        })
    }

    async fn advance(
        &mut self,
        commands: &[StoredCommand],
    ) -> Result<(Option<StoredSnapshot>, Option<Checkpoint>), AdvanceError> {
        let snapshot_every = self.service.config().snapshot_every;
        // Only the last snapshot boundary of the batch is kept: the ones before it are already
        // obsolete by the end of the replay.
        let boundary = if snapshot_every > 0 {
            commands
                .iter()
                .rposition(|command| command.version % snapshot_every == 0)
        } else {
            None
        };

        let snapshot = match boundary {
            Some(boundary) => {
                self.apply(&commands[..=boundary])?;
                let snapshot = self
                    .snapshot_at(commands[boundary].version)
                    .map_err(AdvanceError::Failed)?;
                self.apply(&commands[boundary + 1..])?;
                Some(snapshot)
            }
            None => {
                self.apply(commands)?;
                None
            }
        };

        let (snapshot, checkpoint) = match snapshot {
            Some(snapshot) => match self.service.store_snapshot(&self.stream_id, &snapshot).await {
                SnapshotOutcome::Stored => {
                    let checkpoint = Checkpoint {
                        version: snapshot.version,
                        chain_hash: snapshot.chain_hash,
                        state_hash: snapshot.hash,
                    };
                    (Some(snapshot), Some(checkpoint))
                }
                SnapshotOutcome::Diverged => {
                    return Err(AdvanceError::Reload(format!(
                        "snapshot {} differs from the stored snapshot",
                        snapshot.version
                    )));
                }
                SnapshotOutcome::Skipped => (None, None),
            },
            None => (None, None),
        };
        Ok((snapshot, checkpoint))
    }

    async fn finish(&self, commands: &[StoredCommand]) {
        self.service.trim_inbox(&self.stream_id, commands).await;
        self.service
            .notify_persisted(&self.stream_id, self.replica.version())
            .await;
    }
}

enum AdvanceError {
    Reload(String),
    Failed(CommandStreamError),
}
