use crate::{
    models::{CommandLogError, CommandStreamError},
    repositories::{
        command_inbox::{CommandInboxDb, InboxId},
        command_log::{CommandLogDb, StoredCommand, StreamId},
    },
    services::command_stream::{stream_state::to_envelope, CommandStreamConfig, StreamState},
};
use shine_game::replica::{decode_snapshot, Checkpoint, ReplayState, Replica};
use tracing::instrument;

/// What a sync did to a stream.
#[derive(Debug, Default)]
pub struct SyncOutcome {
    /// The commands confirmed by this sync, in version order and without gaps: the ones this instance
    /// stored and the ones other instances had stored. This is what the subscribers are sent.
    pub confirmed: Vec<StoredCommand>,
    /// The log contradicts the history this instance confirmed. The state is of no further use: the
    /// stream has to be loaded again, and its subscribers synced again.
    pub reload: bool,
    /// There is more to do right away: the inbox had more entries than one sync reads, or some are still
    /// waiting to be stored. The caller should sync again without waiting for a wake.
    pub more: bool,
}

/// What a replay did to a stream.
#[derive(Debug, Default)]
pub struct ReplayOutcome {
    /// The latest checkpoint the replay passed and stored, if any: the state hash at its version, to
    /// be sent to the subscribers.
    pub checkpoint: Option<Checkpoint>,
    /// Another instance stored another state for that version: the instances do not replay alike. The
    /// state is of no further use: the stream has to be loaded again, from the stored snapshot.
    pub reload: bool,
}

enum Step {
    /// Nothing is left to do for now.
    Done,
    /// Something changed: look again.
    Again,
    /// See `SyncOutcome::reload`.
    Reload,
}

/// Works the command streams: loads a stream from the log, and moves the commands waiting in the
/// inbox into the log. It keeps nothing of any stream: the stream is the `StreamState` it is given, and
/// it can work many of them at once.
#[derive(Clone)]
pub struct CommandStreamService<I: CommandInboxDb, L: CommandLogDb> {
    inbox: I,
    log: L,
    config: CommandStreamConfig,
}

impl<I: CommandInboxDb, L: CommandLogDb> CommandStreamService<I, L> {
    pub fn new(inbox: I, log: L, config: CommandStreamConfig) -> Self {
        Self { inbox, log, config }
    }

    /// Loads the stream, creating it if it is new: its latest snapshot and the commands after it,
    /// replayed.
    #[instrument(level = "debug", skip_all, fields(stream_id = %stream_id))]
    pub async fn load<S: ReplayState>(&self, stream_id: &StreamId) -> Result<StreamState<S>, CommandStreamError> {
        let head = self.log.ensure_stream(stream_id, S::KIND).await?;
        let snapshot = self.log.latest_snapshot(stream_id, head.version).await?;
        let (mut replica, mut after) = match &snapshot {
            Some(snapshot) => {
                let state = decode_snapshot::<S>(&snapshot.data)
                    .map_err(|err| CommandStreamError::Corrupt(format!("snapshot {}: {err}", snapshot.version)))?;
                (
                    Replica::from_snapshot(snapshot.version, state, snapshot.chain_hash),
                    snapshot.version,
                )
            }
            None => (Replica::new(), 0),
        };

        let mut tail = Vec::new();
        loop {
            let page = self.log.commands_after(stream_id, after, self.config.page_size).await?;
            let count = page.len() as i64;
            for command in page {
                let envelope = to_envelope::<S>(&command)?;
                replica
                    .apply_next(&envelope)
                    .map_err(|err| CommandStreamError::Corrupt(format!("command {}: {err}", command.version)))?;
                after = command.version;
                tail.push(command);
            }
            if count < self.config.page_size {
                break;
            }
        }

        let stored_inbox = self
            .log
            .last_commands(stream_id, 1)
            .await?
            .last()
            .and_then(|command| command.inbox_id.as_deref())
            .and_then(InboxId::parse);
        Ok(StreamState::new(
            stream_id.clone(),
            replica,
            snapshot,
            tail,
            stored_inbox,
        ))
    }

    /// Moves the commands waiting in the stream's inbox into the log, and brings the stream up to date
    /// with the log: what other instances stored is adopted, and the commands of this instance that they
    /// did not store are appended after it.
    #[instrument(level = "debug", skip_all, fields(stream_id = %state.stream_id()))]
    pub async fn sync<S: ReplayState>(&self, state: &mut StreamState<S>) -> Result<SyncOutcome, CommandStreamError> {
        let mut outcome = SyncOutcome::default();
        let stored_before = state.stored_inbox().cloned();
        // Whether this instance stored any command, as opposed to adopting those of the others.
        let mut stored_own = false;

        let more_in_inbox = self.ingest(state).await?;
        for _ in 0..self.config.max_attempts {
            let step = if state.pending.is_empty() {
                self.follow(state, &mut outcome).await?
            } else {
                self.persist(state, &mut outcome, &mut stored_own).await?
            };
            match step {
                Step::Done => break,
                Step::Again => {}
                Step::Reload => {
                    outcome.reload = true;
                    break;
                }
            }
        }

        outcome.more = !outcome.reload && (more_in_inbox || !state.pending.is_empty());
        if !outcome.reload {
            let advanced = state.stored_inbox() != stored_before.as_ref();
            self.finish(state, advanced, stored_own).await;
        }
        Ok(outcome)
    }

    /// Applies the commands confirmed since the last replay to the stream's replica, and stores the
    /// snapshot of the latest checkpoint version it passed. The commands are in the log already, so a
    /// snapshot that cannot be stored is only logged: the next checkpoint stores one. Replay is off the
    /// path of delivering the commands: it only feeds snapshots and the state a new subscriber is synced
    /// from.
    #[instrument(level = "debug", skip_all, fields(stream_id = %state.stream_id()))]
    pub async fn replay<S: ReplayState>(
        &self,
        state: &mut StreamState<S>,
    ) -> Result<ReplayOutcome, CommandStreamError> {
        let mut outcome = ReplayOutcome::default();
        // Only the latest checkpoint is stored: the ones before it add history, not safety.
        let Some(snapshot) = state.replay(self.config.snapshot_every)? else {
            return Ok(outcome);
        };
        match self.log.try_store_snapshot(state.stream_id(), &snapshot).await {
            Ok(()) => {
                outcome.checkpoint = Some(Checkpoint {
                    version: snapshot.version,
                    chain_hash: snapshot.chain_hash,
                    state_hash: snapshot.hash,
                });
                state.set_snapshot(snapshot);
                if let Err(err) = self.log.prune(state.stream_id(), self.config.keep_snapshots).await {
                    tracing::warn!("[{}] Failed to prune the log: {err:#?}", state.stream_id());
                }
            }
            Err(CommandLogError::SnapshotDiverged) => {
                tracing::error!(
                    "[{}] The state at version {} differs from the stored snapshot",
                    state.stream_id(),
                    snapshot.version
                );
                outcome.reload = true;
            }
            Err(err) => {
                tracing::warn!(
                    "[{}] Failed to store the snapshot of version {}: {err:#?}",
                    state.stream_id(),
                    snapshot.version
                );
            }
        }
        Ok(outcome)
    }

    /// Takes the next entries of the inbox. Returns whether there may be more.
    async fn ingest<S: ReplayState>(&self, state: &mut StreamState<S>) -> Result<bool, CommandStreamError> {
        if state.pending.len() >= self.config.max_pending {
            return Ok(true);
        }
        let batch = self
            .inbox
            .poll(state.stream_id(), &state.cursor, self.config.poll_limit)
            .await?;
        let more = batch.more;
        state.ingest(batch);
        Ok(more)
    }

    /// Nothing is waiting: looks for what other instances stored.
    async fn follow<S: ReplayState>(
        &self,
        state: &mut StreamState<S>,
        outcome: &mut SyncOutcome,
    ) -> Result<Step, CommandStreamError> {
        let Some(head) = self.log.head(state.stream_id()).await? else {
            return Ok(Step::Reload);
        };
        if head.version < state.confirmed_version() {
            return Ok(Step::Reload);
        }
        if head.version == state.confirmed_version() {
            return Ok(if head.chain_hash == state.confirmed_chain_hash() {
                Step::Done
            } else {
                Step::Reload
            });
        }
        self.catch_up(state, outcome).await
    }

    /// Appends the waiting commands after the confirmed history.
    async fn persist<S: ReplayState>(
        &self,
        state: &mut StreamState<S>,
        outcome: &mut SyncOutcome,
        stored_own: &mut bool,
    ) -> Result<Step, CommandStreamError> {
        let (batch, baseline) = state.pending_batch();
        let Some(last) = batch.last() else {
            return Ok(Step::Done);
        };
        match self.log.append(state.stream_id(), &batch, baseline).await {
            // The log ends with the batch: it is in the log as it was sent.
            Ok(head) if head.version == last.version && head.chain_hash == last.chain_hash => {
                state.confirm_pending(&batch);
                *stored_own = true;
                outcome.confirmed.extend(batch);
                Ok(Step::Done)
            }
            // The log is past the batch, which could not be compared with it, or it holds other
            // commands where the batch has its own: other instances were first.
            Ok(_) | Err(CommandLogError::Diverged { .. }) => self.catch_up(state, outcome).await,
            Err(err) => Err(err.into()),
        }
    }

    /// Adopts the commands of the log after the confirmed version, if they continue the confirmed history.
    async fn catch_up<S: ReplayState>(
        &self,
        state: &mut StreamState<S>,
        outcome: &mut SyncOutcome,
    ) -> Result<Step, CommandStreamError> {
        let mut first_page = true;
        loop {
            let page = self
                .log
                .commands_after(state.stream_id(), state.confirmed_version(), self.config.page_size)
                .await?;
            // Nothing after the confirmed version although the log said it was behind it: its history
            // is not ours.
            if page.is_empty() && first_page {
                return Ok(Step::Reload);
            }
            first_page = false;
            let count = page.len() as i64;
            for command in page {
                match state.adopt(command.clone()) {
                    Ok(()) => outcome.confirmed.push(command),
                    Err(err) => {
                        tracing::warn!("[{}] The log does not continue the history: {err:?}", state.stream_id());
                        return Ok(Step::Reload);
                    }
                }
            }
            if count < self.config.page_size {
                return Ok(Step::Again);
            }
        }
    }

    /// Clears the inbox of what is in the log, and tells the other instances. Neither is needed for
    /// correctness, so a failure is only logged: a late trim is harmless, and the others also wake on
    /// their own timer.
    async fn finish<S: ReplayState>(&self, state: &StreamState<S>, advanced: bool, stored_own: bool) {
        if let Some(up_to) = state.stored_inbox().filter(|_| advanced) {
            if let Err(err) = self.inbox.trim(state.stream_id(), up_to).await {
                tracing::warn!("[{}] Failed to trim the inbox: {err:#?}", state.stream_id());
            }
        }
        if stored_own {
            if let Err(err) = self
                .inbox
                .notify_persisted(state.stream_id(), state.confirmed_version())
                .await
            {
                tracing::warn!(
                    "[{}] Failed to announce the persisted version: {err:#?}",
                    state.stream_id()
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "command_stream_service_test.rs"]
mod command_stream_service_test;
