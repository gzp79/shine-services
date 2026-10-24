// Not called by anything outside this module's own tests yet: the service layer that calls through
// `CommandLogDb` is a later round.
#![allow(dead_code)]

use crate::{
    models::CommandLogError,
    repositories::command_log::{ConsistencyReport, StoredCommand, StoredSnapshot, StreamHead, StreamId},
};
use std::future::Future;

/// Access to the command-log data store: a stream's commands and snapshots.
pub trait CommandLogDb: Clone + Send + Sync {
    /// Creates the stream if it does not exist yet, and returns its head either way. Errs if the
    /// stream exists under a different `kind`.
    fn ensure_stream(
        &self,
        stream_id: &StreamId,
        kind: &str,
    ) -> impl Future<Output = Result<StreamHead, CommandLogError>> + Send;

    /// The stream's head, or `None` if it does not exist. Never creates it.
    fn head(&self, stream_id: &StreamId) -> impl Future<Output = Result<Option<StreamHead>, CommandLogError>> + Send;

    /// Stores the rows above the head and proves the overlap with it by chain hash. `commands` must
    /// be a dense, contiguous run of versions; `baseline_hash` is the chain hash through the version
    /// right before the first of them.
    ///
    /// #Return
    /// `Ok(head)` is the stream's head after the call, whether something new was stored or the batch
    /// was already covered. If `head.version` is not beyond the last command, the batch is in the log
    /// exactly as sent. If it is beyond, the log is ahead of the batch and nothing of it could be
    /// compared: the caller has to catch up (`commands_after`) before it trusts the batch.
    /// `Err(CommandLogError::Diverged { head })` if the batch does not match the log, with the head it
    /// was compared with.
    fn append(
        &self,
        stream_id: &StreamId,
        commands: &[StoredCommand],
        baseline_hash: u64,
    ) -> impl Future<Output = Result<StreamHead, CommandLogError>> + Send;

    /// Returns the commands from the given version onwards, up to `limit` rows.
    fn commands_after(
        &self,
        stream_id: &StreamId,
        after: i64,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<StoredCommand>, CommandLogError>> + Send;

    /// The last `count` commands, ordered by version.
    fn last_commands(
        &self,
        stream_id: &StreamId,
        count: i64,
    ) -> impl Future<Output = Result<Vec<StoredCommand>, CommandLogError>> + Send;

    /// Stores a snapshot at `version`.
    ///
    /// # Return
    /// `Ok(())` whether it was newly stored or already covered by
    /// an agreeing one; `Err(CommandLogError::SnapshotDiverged)` if its `chain_hash` does not match the
    /// command already durably stored at that version, or an existing snapshot there disagrees.
    fn try_store_snapshot(
        &self,
        stream_id: &StreamId,
        snapshot: &StoredSnapshot,
    ) -> impl Future<Output = Result<(), CommandLogError>> + Send;

    /// Returns the latest snapshot at or before the given version, if any.
    fn latest_snapshot(
        &self,
        stream_id: &StreamId,
        at_or_before: i64,
    ) -> impl Future<Output = Result<Option<StoredSnapshot>, CommandLogError>> + Send;

    /// The version of the oldest stored snapshot, if there is one.
    fn oldest_snapshot_version(
        &self,
        stream_id: &StreamId,
    ) -> impl Future<Output = Result<Option<i64>, CommandLogError>> + Send;

    /// Keeps the latest `kept_snapshot_count` snapshots and the commands from the oldest of them on, and deletes
    /// the rest in one transaction. Returns the version of the oldest kept snapshot (the horizon), or `None`.
    fn prune(
        &self,
        stream_id: &StreamId,
        kept_snapshot_count: i64,
    ) -> impl Future<Output = Result<Option<i64>, CommandLogError>> + Send;

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
    ) -> impl Future<Output = Result<ConsistencyReport, CommandLogError>> + Send;
}
