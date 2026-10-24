use crate::{
    models::CommandInboxError,
    repositories::command_inbox::{InboxBatch, InboxId, InboxWake, StreamId},
};
use std::future::Future;
use uuid::Uuid;

/// Access to the command inbox: where clients' commands wait, per stream, until an instance sequences
/// them into the command log. It holds nothing durable; what is lost with it is resubmitted by the
/// clients.
pub trait CommandInboxDb: Clone + Send + Sync {
    /// Adds a command to the stream's inbox, renews the inbox's expiry and wakes the listeners of the
    /// stream, all or nothing. Returns the entry's position. Nothing is checked: the same command
    /// pushed twice is two entries.
    fn push(
        &self,
        stream_id: &StreamId,
        command_id: Uuid,
        user_id: Uuid,
        data: &str,
    ) -> impl Future<Output = Result<InboxId, CommandInboxError>> + Send;

    /// Reads up to `limit` entries after `after`, in order. Entries missing a field or holding a
    /// malformed one are skipped and logged; see `InboxBatch::cursor`.
    fn poll(
        &self,
        stream_id: &StreamId,
        after: &InboxId,
        limit: usize,
    ) -> impl Future<Output = Result<InboxBatch, CommandInboxError>> + Send;

    /// Removes the entries up to and including `up_to`, and returns how many there were. A trim point
    /// below an earlier one removes nothing, so instances trimming concurrently cannot undo each other.
    fn trim(
        &self,
        stream_id: &StreamId,
        up_to: &InboxId,
    ) -> impl Future<Output = Result<usize, CommandInboxError>> + Send;

    /// Tells the listeners that the stream is persisted up to `version`.
    fn notify_persisted(
        &self,
        stream_id: &StreamId,
        version: i64,
    ) -> impl Future<Output = Result<(), CommandInboxError>> + Send;

    /// Calls `handler` for every wake of every stream, from a task shared with the other listeners of
    /// the connection: it must not block. At most one handler per connection pool.
    fn listen<F>(&self, handler: F) -> impl Future<Output = Result<(), CommandInboxError>> + Send
    where
        F: Fn(InboxWake) + Send + Sync + 'static;

    /// Stops the handler of `listen`.
    fn unlisten(&self) -> impl Future<Output = Result<(), CommandInboxError>> + Send;
}
