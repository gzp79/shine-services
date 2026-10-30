use crate::{
    models::CommandStreamError,
    repositories::{
        command_inbox::{CommandInboxDb, InboxId},
        command_log::{CommandLogDb, StoredCommand, StoredSnapshot},
    },
    services::command_stream::CommandStreamConfig,
};
use shine_game::replica::StreamId;
use tracing::instrument;

/// A stream as it is stored: the latest snapshot, and the commands after it.
#[derive(Debug, Default)]
pub struct StoredStream {
    pub snapshot: Option<StoredSnapshot>,
    pub tail: Vec<StoredCommand>,
}

/// What the log made of an offered batch.
#[derive(Debug)]
pub enum AppendOutcome {
    /// The batch is in the log, exactly as it was offered: it is the canonical continuation.
    Stored,
    /// Another instance was there first. These are the commands the log holds after the baseline
    /// version instead; they may still start with the offered ones.
    Behind { canonical: Vec<StoredCommand> },
}

/// What the log made of an offered snapshot.
#[derive(Debug)]
pub enum SnapshotOutcome {
    /// The snapshot is stored, or an agreeing one already was.
    Stored,
    /// Another instance stored another state for that version: the instances do not replay alike.
    Diverged,
    /// The store did not take it, for a reason that is not the caller's to tell apart: it is only a
    /// snapshot, so the next one may well succeed.
    Skipped,
}

/// Reads and writes one command stream's durable rows: data in, data out. It holds no stream of its
/// own, so a single instance serves every stream and every caller.
#[derive(Clone)]
pub struct StreamLogService<I: CommandInboxDb, L: CommandLogDb> {
    inbox: I,
    log: L,
    config: CommandStreamConfig,
}

impl<I: CommandInboxDb, L: CommandLogDb> StreamLogService<I, L> {
    pub fn new(inbox: I, log: L, config: CommandStreamConfig) -> Self {
        Self { inbox, log, config }
    }

    pub fn config(&self) -> &CommandStreamConfig {
        &self.config
    }

    /// Reads the stream as it is stored, creating it if it is new.
    #[instrument(level = "debug", skip_all, fields(stream_id = %stream_id))]
    pub async fn load(&self, stream_id: &StreamId) -> Result<StoredStream, CommandStreamError> {
        let head = self.log.ensure_stream(stream_id).await?;
        let snapshot = self.log.latest_snapshot(stream_id, head.version).await?;
        let after = snapshot.as_ref().map(|snapshot| snapshot.version).unwrap_or(0);
        let tail = self.commands_after(stream_id, after).await?;
        Ok(StoredStream { snapshot, tail })
    }

    /// Every command the log holds after the given version.
    #[instrument(level = "debug", skip_all, fields(stream_id = %stream_id, after = after))]
    pub async fn commands_after(
        &self,
        stream_id: &StreamId,
        after: i64,
    ) -> Result<Vec<StoredCommand>, CommandStreamError> {
        let mut after = after;
        let mut commands = Vec::new();
        loop {
            let page = self.log.commands_after(stream_id, after, self.config.page_size).await?;
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

    /// Offers a batch as the continuation of the stream at `baseline_version`, whose chain hash is
    /// `baseline_chain_hash`. The log decides: either the batch is the continuation, or the caller is
    /// told what the continuation is instead.
    #[instrument(level = "debug", skip_all, fields(stream_id = %stream_id, baseline_version = baseline_version))]
    pub async fn append(
        &self,
        stream_id: &StreamId,
        commands: &[StoredCommand],
        baseline_version: i64,
        baseline_chain_hash: u64,
    ) -> Result<AppendOutcome, CommandStreamError> {
        let Some(last) = commands.last() else {
            return Ok(AppendOutcome::Stored);
        };
        match self.log.append(stream_id, commands, baseline_chain_hash).await {
            Ok(head) if head.version == last.version && head.chain_hash == last.chain_hash => Ok(AppendOutcome::Stored),
            Ok(_) | Err(CommandStreamError::LogDiverged { .. }) => {
                let canonical = self.commands_after(stream_id, baseline_version).await?;
                Ok(AppendOutcome::Behind { canonical })
            }
            Err(error) => Err(error),
        }
    }

    /// Stores a snapshot and prunes what it made obsolete. Pruning is best effort: it is only space.
    #[instrument(level = "debug", skip_all, fields(stream_id = %stream_id, version = snapshot.version))]
    pub async fn store_snapshot(&self, stream_id: &StreamId, snapshot: &StoredSnapshot) -> SnapshotOutcome {
        match self.log.try_store_snapshot(stream_id, snapshot).await {
            Ok(()) => {
                if let Err(error) = self.log.prune(stream_id, self.config.keep_snapshots).await {
                    tracing::warn!("[{stream_id}] Failed to prune the log: {error:#?}");
                }
                SnapshotOutcome::Stored
            }
            Err(CommandStreamError::SnapshotDiverged) => SnapshotOutcome::Diverged,
            Err(error) => {
                tracing::warn!(
                    "[{stream_id}] Failed to store the snapshot of version {}: {error:#?}",
                    snapshot.version
                );
                SnapshotOutcome::Skipped
            }
        }
    }

    /// Drops the inbox entries the given commands came from: they are durable now. Best effort, as a
    /// missed trim only leaves entries that are stored again, which the log refuses a second time.
    #[instrument(level = "debug", skip_all, fields(stream_id = %stream_id))]
    pub async fn trim_inbox(&self, stream_id: &StreamId, commands: &[StoredCommand]) {
        let trim_through = commands
            .iter()
            .filter_map(|command| command.inbox_id.as_deref().and_then(InboxId::parse))
            .max();
        if let Some(trim_through) = trim_through {
            if let Err(error) = self.inbox.trim(stream_id, &trim_through).await {
                tracing::warn!("[{stream_id}] Failed to trim the inbox: {error:#?}");
            }
        }
    }

    /// Tells the other instances how far the stream is durable. Best effort: a missed wake is caught
    /// by the next one.
    #[instrument(level = "debug", skip_all, fields(stream_id = %stream_id, version = version))]
    pub async fn notify_persisted(&self, stream_id: &StreamId, version: i64) {
        if let Err(error) = self.inbox.notify_persisted(stream_id, version).await {
            tracing::warn!("[{stream_id}] Failed to announce the persisted version: {error:#?}");
        }
    }
}
