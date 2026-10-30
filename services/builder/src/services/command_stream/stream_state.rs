use crate::models::CommandStreamError;
use crate::repositories::command_inbox::{InboxBatch, InboxId};
use crate::repositories::command_log::{StoredCommand, StoredSnapshot};
use shine_game::replica::{
    decode_command, encode_command, encode_snapshot, fold_chain_hash, Command, CommandEnvelope, CommandHeader,
    ReplayState, Replica, StreamId,
};
use uuid::Uuid;

/// A command taken from the inbox, waiting to be persisted. It has no version: that is decided when
/// it is appended, from the last confirmed version.
pub struct Pending {
    pub inbox_id: InboxId,
    pub command_id: Uuid,
    pub user_id: Uuid,
    /// The canonical command JSON.
    pub data: String,
    pub command_hash: u64,
}

/// Why a command of the log could not be adopted.
#[derive(Debug, PartialEq, Eq)]
pub enum AdoptError {
    /// The command is not the next version.
    Gap,
    /// The command does not continue the chain of the confirmed history.
    Diverged,
}

/// One stream on one instance. It belongs to the task that works the stream, and the service takes it
/// by `&mut`: the service itself keeps nothing of any stream.
///
/// The history is split at the **confirmed** version, the last version checked against the log. The
/// commands above it are `pending`: they have no version yet, and get one the next time they are
/// appended.
pub struct StreamState<S: ReplayState> {
    stream_id: StreamId,
    /// The replayed state. It trails the confirmed version: the commands confirmed since the load are
    /// applied later, off the path of delivering them.
    replica: Replica<S>,
    snapshot: Option<StoredSnapshot>,
    /// The confirmed commands after the snapshot.
    tail: Vec<StoredCommand>,
    confirmed_version: i64,
    confirmed_chain_hash: u64,
    pub(super) pending: Vec<Pending>,
    /// Where the inbox is read from next.
    pub(super) cursor: InboxId,
    /// The latest inbox position known to be in the log. Commands are stored in inbox order, so every
    /// inbox entry at or before it is stored (or was skipped by whoever stored the ones after it).
    stored_inbox: Option<InboxId>,
}

impl<S: ReplayState> StreamState<S> {
    pub(super) fn new(
        stream_id: StreamId,
        replica: Replica<S>,
        snapshot: Option<StoredSnapshot>,
        tail: Vec<StoredCommand>,
        stored_inbox: Option<InboxId>,
    ) -> Self {
        Self {
            stream_id,
            confirmed_version: replica.version(),
            confirmed_chain_hash: replica.chain_hash(),
            replica,
            snapshot,
            tail,
            pending: Vec::new(),
            cursor: stored_inbox.clone().unwrap_or(InboxId::START),
            stored_inbox,
        }
    }

    pub fn stream_id(&self) -> &StreamId {
        &self.stream_id
    }

    pub fn replica(&self) -> &Replica<S> {
        &self.replica
    }

    pub fn snapshot(&self) -> Option<&StoredSnapshot> {
        self.snapshot.as_ref()
    }

    pub fn tail(&self) -> &[StoredCommand] {
        &self.tail
    }

    pub fn confirmed_version(&self) -> i64 {
        self.confirmed_version
    }

    pub fn confirmed_chain_hash(&self) -> u64 {
        self.confirmed_chain_hash
    }

    /// Applies the confirmed commands the replica has not seen yet. Returns the snapshot of the latest
    /// checkpoint version it passed (a multiple of `snapshot_every`), taken at exactly that version even
    /// when more commands follow.
    pub fn replay(&mut self, snapshot_every: i64) -> Result<Option<StoredSnapshot>, CommandStreamError> {
        let mut snapshot = None;
        let start = self
            .tail
            .iter()
            .position(|command| command.version > self.replica.version())
            .unwrap_or(self.tail.len());
        for command in &self.tail[start..] {
            let envelope = command.to_envelope::<S>()?;
            self.replica
                .apply_next(&envelope)
                .map_err(|err| CommandStreamError::Corrupt(format!("command {}: {err}", command.version)))?;
            if snapshot_every > 0 && command.version % snapshot_every == 0 {
                let data = encode_snapshot(self.replica.state())
                    .map_err(|err| CommandStreamError::Corrupt(format!("snapshot {}: {err}", command.version)))?;
                snapshot = Some(StoredSnapshot {
                    version: command.version,
                    hash: self.replica.hash(),
                    chain_hash: self.replica.chain_hash(),
                    data,
                });
            }
        }
        Ok(snapshot)
    }

    /// Makes `snapshot` the stream's latest one; the tail goes on after it.
    pub fn set_snapshot(&mut self, snapshot: StoredSnapshot) {
        self.tail.retain(|command| command.version > snapshot.version);
        self.snapshot = Some(snapshot);
    }

    /// The latest inbox position known to be in the log.
    pub fn stored_inbox(&self) -> Option<&InboxId> {
        self.stored_inbox.as_ref()
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Takes the entries of a poll as pending commands, in inbox order. An entry that is already in the
    /// log, one that does not decode and one that is not well formed are dropped. Command ids are the
    /// client's own and are not checked: a command sent twice is two commands.
    pub fn ingest(&mut self, batch: InboxBatch) {
        for entry in batch.entries {
            if self
                .stored_inbox
                .as_ref()
                .is_some_and(|stored| entry.inbox_id <= *stored)
            {
                continue;
            }
            let command = match decode_command::<S>(&entry.data) {
                Ok(command) if S::is_well_formed(&command) => command,
                _ => {
                    tracing::warn!("[{}] Dropping malformed command {}", self.stream_id, entry.command_id);
                    continue;
                }
            };
            let Ok(data) = encode_command::<S>(&command) else {
                continue;
            };
            self.pending.push(Pending {
                inbox_id: entry.inbox_id,
                command_id: entry.command_id,
                user_id: entry.user_id,
                data,
                command_hash: command.content_hash(),
            });
        }
        self.cursor = batch.cursor;
    }

    /// The pending commands as they would be stored on top of the confirmed history, with the chain hash
    /// before the first of them.
    pub fn pending_batch(&self) -> (Vec<StoredCommand>, u64) {
        let mut version = self.confirmed_version;
        let mut chain_hash = self.confirmed_chain_hash;
        let commands = self
            .pending
            .iter()
            .map(|pending| {
                version += 1;
                chain_hash = fold_chain_hash(chain_hash, version, pending.command_hash);
                StoredCommand {
                    version,
                    command_id: pending.command_id,
                    user_id: pending.user_id,
                    data: pending.data.clone(),
                    command_hash: pending.command_hash,
                    chain_hash,
                    inbox_id: Some(pending.inbox_id.to_string()),
                }
            })
            .collect();
        (commands, self.confirmed_chain_hash)
    }

    /// The pending commands were stored as `batch` (what `pending_batch` returned): they are confirmed.
    pub fn confirm_pending(&mut self, batch: &[StoredCommand]) {
        let Some(last) = batch.last() else {
            return;
        };
        self.confirmed_version = last.version;
        self.confirmed_chain_hash = last.chain_hash;
        self.tail.extend(batch.iter().cloned());
        if let Some(last_pending) = self.pending.pop() {
            self.stored_inbox = self.stored_inbox.take().max(Some(last_pending.inbox_id));
        }
        self.pending.clear();
    }

    /// Takes over a command another instance stored, if it continues the confirmed history. The pending
    /// commands at or before its inbox position are done: the other instance stored them, or skipped them.
    pub fn adopt(&mut self, command: StoredCommand) -> Result<(), AdoptError> {
        if command.version != self.confirmed_version + 1 {
            return Err(AdoptError::Gap);
        }
        if fold_chain_hash(self.confirmed_chain_hash, command.version, command.command_hash) != command.chain_hash {
            return Err(AdoptError::Diverged);
        }
        self.confirmed_version = command.version;
        self.confirmed_chain_hash = command.chain_hash;
        if let Some(position) = command.inbox_id.as_deref().and_then(InboxId::parse) {
            self.pending.retain(|pending| pending.inbox_id > position);
            if self.cursor < position {
                self.cursor = position.clone();
            }
            self.stored_inbox = self.stored_inbox.take().max(Some(position));
        }
        self.tail.push(command);
        Ok(())
    }
}
