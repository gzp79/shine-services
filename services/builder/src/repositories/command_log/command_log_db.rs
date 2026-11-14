use crate::{
    models::CommandStreamError,
    repositories::command_log::{ConsistencyReport, StoredCommand, StoredSnapshot, StreamHead},
};
use shine_game::replica::StreamId;
use std::future::Future;

/// Access to the command-log data store: a stream's commands and snapshots.
pub trait CommandLogDb: Clone + Send + Sync {
    /// Creates the stream if it does not exist yet, and returns its head either way.
    fn ensure_stream(
        &self,
        stream_id: &StreamId,
    ) -> impl Future<Output = Result<StreamHead, CommandStreamError>> + Send;

    /// The stream's head, or `None` if it does not exist. Never creates it.
    fn head(&self, stream_id: &StreamId)
        -> impl Future<Output = Result<Option<StreamHead>, CommandStreamError>> + Send;

    /// Stores the rows above the head and checks retained overlapping rows by chain and command hash.
    /// An inbox position already stored at another version is diverged. `commands` must
    /// be a dense, contiguous run of versions; `baseline_hash` is the chain hash through the version
    /// right before the first of them.
    ///
    /// #Return
    /// `Ok(head)` is the stream's head after the call, whether something new was stored or the batch
    /// was already covered. Retained overlapping commands must match both hashes; pruned commands
    /// cannot be verified and are never reinserted. If the returned head is beyond the batch, the
    /// caller has to catch up (`commands_after`).
    /// `Err(CommandStreamError::LogDiverged { head })` if the batch does not match the log, with the head it
    /// was compared with.
    fn append(
        &self,
        stream_id: &StreamId,
        commands: &[StoredCommand],
        baseline_hash: u64,
    ) -> impl Future<Output = Result<StreamHead, CommandStreamError>> + Send;

    /// Returns the commands from the given version onwards, up to `limit` rows.
    fn commands_after(
        &self,
        stream_id: &StreamId,
        after: i64,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<StoredCommand>, CommandStreamError>> + Send;

    /// The last `count` commands, ordered by version.
    fn last_commands(
        &self,
        stream_id: &StreamId,
        count: i64,
    ) -> impl Future<Output = Result<Vec<StoredCommand>, CommandStreamError>> + Send;

    /// Stores a snapshot at `version`.
    ///
    /// # Return
    /// `Ok(())` whether it was newly stored or already covered by
    /// an agreeing one; `Err(CommandStreamError::SnapshotDiverged)` if its `chain_hash` does not match the
    /// command already durably stored at that version, or an existing snapshot there disagrees.
    fn try_store_snapshot(
        &self,
        stream_id: &StreamId,
        snapshot: &StoredSnapshot,
    ) -> impl Future<Output = Result<(), CommandStreamError>> + Send;

    /// Returns the latest snapshot at or before the given version, if any.
    fn latest_snapshot(
        &self,
        stream_id: &StreamId,
        at_or_before: i64,
    ) -> impl Future<Output = Result<Option<StoredSnapshot>, CommandStreamError>> + Send;

    /// The version of the oldest stored snapshot, if there is one.
    fn oldest_snapshot_version(
        &self,
        stream_id: &StreamId,
    ) -> impl Future<Output = Result<Option<i64>, CommandStreamError>> + Send;

    /// Keeps the latest `kept_snapshot_count` snapshots and the commands from the oldest of them on, and deletes
    /// the rest in one transaction. Returns the version of the oldest kept snapshot (the horizon), or `None`.
    fn prune(
        &self,
        stream_id: &StreamId,
        kept_snapshot_count: i64,
    ) -> impl Future<Output = Result<Option<i64>, CommandStreamError>> + Send;

    /// Verifies the stream from its stored rows, up to the head version read at the start:
    /// - the head agrees with the last command.
    /// - the commands form one unbroken chain,
    /// - every snapshot agrees with the command at its version
    ///
    /// Reading the head first and bounding the whole check to it means a concurrent append is
    /// simply not part of this pass rather than a false positive: safe to call anytime, without
    /// blocking writers, even under load.
    fn check_consistency(
        &self,
        stream_id: &StreamId,
    ) -> impl Future<Output = Result<ConsistencyReport, CommandStreamError>> + Send;
}
