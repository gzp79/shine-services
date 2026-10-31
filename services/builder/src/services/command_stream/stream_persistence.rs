use crate::{
    models::CommandStreamError,
    repositories::{
        command_inbox::{CommandInboxDb, InboxId},
        command_log::{CommandLogDb, StoredCommand, StoredSnapshot},
    },
    services::command_stream::stream_log_service::{AppendOutcome, SnapshotOutcome, StreamLogService},
};
use shine_game::replica::{decode_snapshot, encode_snapshot, Checkpoint, ReplayState, Replica, StreamId};
use std::sync::Arc;

/// The canonical result of a persistence attempt.
#[derive(Debug)]
pub enum PersistResult {
    Current,
    Advanced {
        canonical: Arc<[StoredCommand]>,
        snapshot: Option<StoredSnapshot>,
        checkpoint: Option<Checkpoint>,
    },
    Reloaded {
        snapshot: Option<StoredSnapshot>,
        tail: Arc<[StoredCommand]>,
        stored_inbox: Option<InboxId>,
        reason: String,
    },
    Failed {
        error: CommandStreamError,
    },
}

enum AdvanceError {
    Reload(String),
    Failed(CommandStreamError),
}

/// The durable side of one stream: appends the optimistic batches to the log, replays the canonical
/// history onto its private replica, stores the snapshots and clears the inbox behind them.
///
/// It owns no task and no channel: every entry point is an ordinary method, so it can be driven
/// straight from a test. `StreamWorker` is what gives it a task of its own in production.
pub struct StreamPersistence<S, I, L>
where
    S: ReplayState,
    I: CommandInboxDb,
    L: CommandLogDb,
{
    stream_id: StreamId,
    service: StreamLogService<I, L>,
    replica: Replica<S>,
}

impl<S, I, L> StreamPersistence<S, I, L>
where
    S: ReplayState,
    I: CommandInboxDb + 'static,
    L: CommandLogDb + 'static,
{
    pub fn new(stream_id: StreamId, service: StreamLogService<I, L>) -> Self {
        Self {
            stream_id,
            service,
            replica: Replica::new(),
        }
    }

    pub fn stream_id(&self) -> &StreamId {
        &self.stream_id
    }

    pub fn version(&self) -> i64 {
        self.replica.version()
    }

    pub fn chain_hash(&self) -> u64 {
        self.replica.chain_hash()
    }

    pub async fn persist(
        &mut self,
        baseline_version: i64,
        baseline_chain_hash: u64,
        commands: Arc<[StoredCommand]>,
    ) -> PersistResult {
        if baseline_version != self.replica.version() || baseline_chain_hash != self.replica.chain_hash() {
            return self
                .reload(format!(
                    "persistence baseline {} does not match canonical version {}",
                    baseline_version,
                    self.replica.version()
                ))
                .await;
        }
        if commands.is_empty() {
            return PersistResult::Advanced {
                canonical: Arc::from([]),
                snapshot: None,
                checkpoint: None,
            };
        }
        let proposed = commands.clone();

        let canonical = match self
            .service
            .append(&self.stream_id, &commands, baseline_version, baseline_chain_hash)
            .await
        {
            Ok(AppendOutcome::Stored) => commands,
            Ok(AppendOutcome::Behind { canonical }) if !canonical.is_empty() => canonical.into(),
            Ok(AppendOutcome::Behind { .. }) => {
                return self
                    .reload("the command log advanced without a readable continuation")
                    .await;
            }
            Err(error) => return PersistResult::Failed { error },
        };
        let shared = canonical.len().min(proposed.len());
        if canonical[..shared] != proposed[..shared] {
            return self
                .reload("canonical commands differ from the optimistic history")
                .await;
        }

        match self.advance(&canonical).await {
            Ok((snapshot, checkpoint)) => {
                self.finish(&canonical).await;
                PersistResult::Advanced {
                    canonical,
                    snapshot,
                    checkpoint,
                }
            }
            Err(AdvanceError::Reload(reason)) => self.reload(reason).await,
            Err(AdvanceError::Failed(error)) => PersistResult::Failed { error },
        }
    }

    /// Checks whether the durable stream still has the head the actor knows. A peer may have advanced
    /// and trimmed the inbox before this worker observed its notification, so a mismatch reloads the
    /// canonical history even when there are no local commands to persist.
    pub async fn reconcile(&mut self, version: i64, chain_hash: u64) -> PersistResult {
        match self.service.head(&self.stream_id).await {
            Ok(Some(head)) if head.version == version && head.chain_hash == chain_hash => PersistResult::Current,
            Ok(_) => {
                self.reload("the durable head advanced without a local persistence")
                    .await
            }
            Err(error) => PersistResult::Failed { error },
        }
    }

    /// Builds the replica from the log: this instance has no history yet, or no longer agrees with the
    /// log. The same work either way, so a stream starts from a reload like any other.
    pub async fn reload(&mut self, reason: impl Into<String>) -> PersistResult {
        match self.load_canonical().await {
            Ok((snapshot, tail, stored_inbox, replica)) => {
                self.replica = replica;
                PersistResult::Reloaded {
                    snapshot,
                    tail: tail.into(),
                    stored_inbox,
                    reason: reason.into(),
                }
            }
            Err(error) => PersistResult::Failed { error },
        }
    }

    #[allow(clippy::type_complexity)]
    async fn load_canonical(
        &self,
    ) -> Result<(Option<StoredSnapshot>, Vec<StoredCommand>, Option<InboxId>, Replica<S>), CommandStreamError> {
        let stored = self.service.load(&self.stream_id).await?;
        let replica = Self::rebuild(stored.snapshot.as_ref(), &stored.tail)?;
        Ok((stored.snapshot, stored.tail, stored.stored_inbox, replica))
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

    /// Clears what is durable from the inbox and tells the other instances how far the log goes.
    async fn finish(&self, commands: &[StoredCommand]) {
        self.service.trim_inbox(&self.stream_id, commands).await;
        self.service
            .notify_persisted(&self.stream_id, self.replica.version())
            .await;
    }
}
