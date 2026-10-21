//! The replay state of one stream worker, without IO: unit-tested without Redis or Postgres.

use crate::{
    models::messages::{StreamCommand, StreamMessage, StreamResume, StreamSnapshot},
    repositories::command_stream::redis::{RedisStreamEntry, RedisStreamMeta},
    settings::KindSettings,
};
use shine_game::replay::{decode_command, encode_snapshot, ApplyOutcome, CommandEnvelope, ReplayState, Replica};
use shine_infra::db::command_log::{AppendOutcome, SnapshotOutcome, StoredCommand, StreamId};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};
use uuid::Uuid;

/// Max commands per egress frame.
pub const FRAME_COMMANDS: usize = 256;

/// Captured checkpoints kept while waiting for persistence; only the newest ready one is resolved.
const MAX_CAPTURED: usize = 2;

/// Why the worker must rebuild its state, or stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Rebuild from Postgres.
    Reload(&'static str),
    /// Drop the Redis copy if it is still at `generation`, then rebuild from Postgres.
    Reset { generation: i64, reason: &'static str },
    /// A stored command cannot be decoded: an inconsistent store, or a missing validation at ingest.
    /// The replay needs every version, so the stream cannot continue; rebuilding would not help.
    Undecodable { version: i64 },
}

/// The verdict on the Redis meta of a read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observed {
    Current,
    /// Redis moved to a newer generation while commands applied from the old one are not proven
    /// persisted: append them and pass the outcome to `prove_and_adopt`.
    NeedsProof(i64),
}

/// A state captured at a checkpoint version, waiting for the version to be persisted.
#[derive(Clone, Debug)]
pub struct Captured {
    pub version: i64,
    pub hash: u64,
    pub data: Arc<[u8]>,
}

/// Replica, persistence progress and subscriber cursors of one stream on one instance.
pub struct StreamCore<S: ReplayState> {
    stream_id: StreamId,
    kind: KindSettings,
    replica: Replica<S>,
    generation: i64,
    /// Highest applied version known to be in Postgres.
    persisted: i64,
    /// Highest version this worker recorded as persisted in Redis.
    marked: i64,
    /// Version and command id of the last applied command, checked against Redis on every read.
    tip: Option<(i64, Uuid)>,
    /// Applied (version, command id) above `marked`: the persisted mark is recorded by content.
    recent: VecDeque<(i64, Uuid)>,
    authoritative: Option<StreamSnapshot>,
    /// Applied commands above min(authoritative version, fanned_out): for `Sync`, fan-out, resume and rebase.
    since_snapshot: Vec<StreamCommand>,
    /// Commands applied from Redis that are not proven to be in Postgres, ascending.
    unpersisted: Vec<StoredCommand>,
    captured: VecDeque<Captured>,
    checkpoint_to_push: Option<(i64, u64)>,
    /// Subscribers that received every version up to `fanned_out`.
    synced: HashSet<Uuid>,
    /// Subscribers waiting for a `Sync`.
    pending_sync: HashSet<Uuid>,
    /// Subscribers that want to continue from a position, if the core can serve it when it delivers.
    pending_resume: HashMap<Uuid, StreamResume>,
    fanned_out: i64,
}

fn decode<S: ReplayState>(command: &StoredCommand) -> Result<CommandEnvelope<S::Command>, Fault> {
    let decoded = decode_command::<S>(&command.data).map_err(|_| Fault::Undecodable { version: command.version })?;
    Ok(CommandEnvelope {
        version: command.version,
        command_id: command.command_id,
        user_id: command.user_id,
        command: decoded,
    })
}

impl<S: ReplayState> StreamCore<S> {
    pub fn new(stream_id: StreamId, kind: KindSettings) -> Self {
        Self {
            stream_id,
            kind,
            replica: Replica::new(),
            generation: 0,
            persisted: 0,
            marked: 0,
            tip: None,
            recent: VecDeque::new(),
            authoritative: None,
            since_snapshot: Vec::new(),
            unpersisted: Vec::new(),
            captured: VecDeque::new(),
            checkpoint_to_push: None,
            synced: HashSet::new(),
            pending_sync: HashSet::new(),
            pending_resume: HashMap::new(),
            fanned_out: 0,
        }
    }

    pub fn version(&self) -> i64 {
        self.replica.version()
    }

    pub fn generation(&self) -> i64 {
        self.generation
    }

    /// The version of the latest authoritative snapshot (0 without one): the oldest version a resume
    /// can continue from.
    pub fn base_version(&self) -> i64 {
        self.authoritative.as_ref().map_or(0, |a| a.version)
    }

    #[cfg(test)]
    pub fn state(&self) -> &S {
        self.replica.state()
    }

    pub fn unpersisted(&self) -> &[StoredCommand] {
        &self.unpersisted
    }

    /// A subscriber that wants the full state: a `Sync` with the snapshot and the commands after it.
    pub fn add_subscriber(&mut self, connection_id: Uuid) {
        self.forget(connection_id);
        self.pending_sync.insert(connection_id);
    }

    /// A subscriber that continues from `resume`. If the core can serve the position when it delivers
    /// (same generation, not older than its snapshot, not above its head) the subscriber gets only the
    /// commands after it, otherwise a full `Sync`.
    pub fn add_subscriber_resuming(&mut self, connection_id: Uuid, resume: StreamResume) {
        self.forget(connection_id);
        self.pending_resume.insert(connection_id, resume);
    }

    pub fn remove_subscriber(&mut self, connection_id: Uuid) {
        self.forget(connection_id);
    }

    fn forget(&mut self, connection_id: Uuid) {
        self.synced.remove(&connection_id);
        self.pending_sync.remove(&connection_id);
        self.pending_resume.remove(&connection_id);
    }

    /// Starts a rebuild from `snapshot`, the latest stored one with its decoded state. Every
    /// subscriber gets a full `Sync` once loaded, whatever it asked for.
    pub fn begin_load(&mut self, snapshot: Option<(StreamSnapshot, S)>) {
        match snapshot {
            Some((snapshot, state)) => {
                self.replica = Replica::from_snapshot(snapshot.version, state);
                self.authoritative = Some(snapshot);
            }
            None => {
                self.replica = Replica::new();
                self.authoritative = None;
            }
        }
        self.since_snapshot.clear();
        self.unpersisted.clear();
        self.captured.clear();
        self.checkpoint_to_push = None;
        self.persisted = self.replica.version();
        self.marked = 0;
        self.tip = None;
        self.recent.clear();
        self.fanned_out = self.replica.version();
        let synced = std::mem::take(&mut self.synced);
        self.pending_sync.extend(synced);
        let resuming = std::mem::take(&mut self.pending_resume);
        self.pending_sync.extend(resuming.into_keys());
    }

    /// Ends a rebuild: the Postgres log is applied; `meta` is the Redis copy to continue from.
    pub fn finish_load(&mut self, meta: &RedisStreamMeta) {
        self.persisted = self.replica.version();
        self.generation = meta.generation;
    }

    /// Whether the load ended on a snapshot without a later command, so the tip's command id is unknown.
    pub fn needs_tip(&self) -> bool {
        self.tip.is_none() && self.replica.version() > 0
    }

    /// Sets the command id of the replica's version, read from Postgres after a load.
    pub fn set_tip(&mut self, version: i64, command_id: Uuid) {
        if version == self.replica.version() {
            self.tip = Some((version, command_id));
        }
    }

    /// Applies the next command; `persisted` when it was read from Postgres.
    pub fn apply(&mut self, command: StoredCommand, persisted: bool) -> Result<(), Fault> {
        let envelope = decode::<S>(&command)?;
        match self.replica.apply_next(&envelope) {
            Ok(ApplyOutcome::AlreadyApplied) => return Ok(()),
            Ok(ApplyOutcome::Applied) => {}
            Err(_) => return Err(Fault::Reload("gap in the command log")),
        }
        if let Some(captured) = capture(&self.stream_id, &self.kind, &self.replica) {
            push_captured(&mut self.captured, captured);
        }
        self.since_snapshot.push(StreamCommand {
            version: command.version,
            command_id: command.command_id,
            user_id: command.user_id,
            data: Arc::from(command.data.as_str()),
        });
        self.tip = Some((command.version, command.command_id));
        self.recent.push_back((command.version, command.command_id));
        if persisted {
            self.persisted = self.persisted.max(command.version);
        } else {
            self.unpersisted.push(command);
        }
        Ok(())
    }

    /// Checks the Redis meta that came with a read, and `tip`, the entry Redis holds at the replica's
    /// version (the read starts there).
    pub fn observe(&mut self, meta: &RedisStreamMeta, tip: Option<&RedisStreamEntry>) -> Result<Observed, Fault> {
        if meta.generation < self.generation {
            return Err(Fault::Reset {
                generation: meta.generation,
                reason: "Redis went back to an older generation",
            });
        }
        if meta.generation > self.generation {
            if !self.unpersisted.is_empty() {
                return Ok(Observed::NeedsProof(meta.generation));
            }
            self.adopt(meta.generation);
        }
        if meta.version < self.replica.version() {
            return Err(Fault::Reset {
                generation: meta.generation,
                reason: "Redis is behind the applied commands",
            });
        }
        self.verify_tip(meta, tip)?;
        Ok(Observed::Current)
    }

    /// Redis must still hold the command this replica applied last, unless that version was persisted
    /// and trimmed (only persisted versions are trimmed). Another command there means Redis went back
    /// in time and reassigned the version (restart from a dump, failover); a missing unpersisted one
    /// means Redis lost it. Counting versions alone cannot see a reassignment that already caught up.
    fn verify_tip(&self, meta: &RedisStreamMeta, tip: Option<&RedisStreamEntry>) -> Result<(), Fault> {
        let Some((version, command_id)) = self.tip else {
            return Ok(());
        };
        match tip.filter(|entry| entry.version == version) {
            Some(entry) if entry.command_id == command_id => Ok(()),
            Some(_) => Err(Fault::Reset {
                generation: meta.generation,
                reason: "Redis reassigned an applied version",
            }),
            None if version <= meta.persisted => Ok(()),
            None => Err(Fault::Reset {
                generation: meta.generation,
                reason: "Redis lost an applied version",
            }),
        }
    }

    /// Finishes a generation change with the outcome of appending the unpersisted commands.
    pub fn prove_and_adopt(&mut self, generation: i64, outcome: AppendOutcome) -> Result<(), Fault> {
        match outcome {
            AppendOutcome::UpToDate { version } if version >= self.replica.version() => {
                self.unpersisted.clear();
                self.persisted = self.replica.version();
                self.adopt(generation);
                Ok(())
            }
            _ => Err(Fault::Reload("Redis changed generation with commands not in Postgres")),
        }
    }

    fn adopt(&mut self, generation: i64) {
        self.generation = generation;
        self.marked = 0;
    }

    /// Applies the outcome of appending `unpersisted` to Postgres.
    pub fn on_append(&mut self, outcome: AppendOutcome) -> Result<(), Fault> {
        match outcome {
            AppendOutcome::Appended { version } | AppendOutcome::UpToDate { version } => {
                self.unpersisted.retain(|c| c.version > version);
                self.persisted = self.persisted.max(version.min(self.replica.version()));
                Ok(())
            }
            // RESET is generation-guarded: when Redis already moved on, it is a no-op.
            AppendOutcome::StaleGeneration { .. } => Err(Fault::Reset {
                generation: self.generation,
                reason: "Redis generation differs from Postgres",
            }),
            AppendOutcome::Gap { .. } => Err(Fault::Reload("Postgres is behind the applied commands")),
            AppendOutcome::Diverged { .. } => Err(Fault::Reset {
                generation: self.generation,
                reason: "Redis diverged from Postgres",
            }),
        }
    }

    /// The version to record as persisted in Redis, with the command applied there, when it is ahead
    /// of what is recorded.
    pub fn mark_due(&self) -> Option<(i64, Uuid)> {
        if self.persisted <= self.marked {
            return None;
        }
        let at = self.recent.partition_point(|(version, _)| *version < self.persisted);
        self.recent
            .get(at)
            .filter(|(version, _)| *version == self.persisted)
            .copied()
    }

    pub fn on_marked(&mut self, version: i64) {
        self.marked = self.marked.max(version);
        while self.recent.front().is_some_and(|(v, _)| *v <= self.marked) {
            self.recent.pop_front();
        }
    }

    /// The newest captured checkpoint that is persisted; older ready ones are dropped.
    pub fn ready_checkpoint(&mut self) -> Option<Captured> {
        while self.captured.len() > 1 && self.captured[1].version <= self.persisted {
            self.captured.pop_front();
        }
        self.captured.front().filter(|c| c.version <= self.persisted).cloned()
    }

    /// Applies the outcome of storing `checkpoint`. Returns the version to rebase onto when another
    /// instance stored a different state there first.
    pub fn on_snapshot(&mut self, checkpoint: &Captured, outcome: SnapshotOutcome) -> Option<i64> {
        if self.captured.front().map(|c| c.version) == Some(checkpoint.version) {
            self.captured.pop_front();
        }
        let agreed = match outcome {
            SnapshotOutcome::Stored => true,
            SnapshotOutcome::Exists { hash } => hash == checkpoint.hash,
            SnapshotOutcome::Superseded => return None,
        };
        if !agreed {
            return Some(checkpoint.version);
        }
        if self
            .authoritative
            .as_ref()
            .is_some_and(|a| a.version >= checkpoint.version)
        {
            return None;
        }
        self.set_authoritative(StreamSnapshot {
            version: checkpoint.version,
            hash: checkpoint.hash,
            data: checkpoint.data.clone(),
        });
        None
    }

    /// Replaces the state with `state`, the authoritative one at `snapshot.version`, and replays the
    /// newer applied commands on it. Atomic: on error nothing changed.
    pub fn rebase(&mut self, snapshot: StreamSnapshot, state: S) -> Result<(), Fault> {
        let mut replica = Replica::from_snapshot(snapshot.version, state);
        let mut captured = VecDeque::new();
        for command in self.since_snapshot.iter().filter(|c| c.version > snapshot.version) {
            let stored = StoredCommand {
                version: command.version,
                command_id: command.command_id,
                user_id: command.user_id,
                generation: self.generation,
                data: command.data.to_string(),
            };
            let envelope = decode::<S>(&stored)?;
            if replica.apply_next(&envelope).is_err() {
                return Err(Fault::Reload("gap while rebasing"));
            }
            if let Some(checkpoint) = capture(&self.stream_id, &self.kind, &replica) {
                push_captured(&mut captured, checkpoint);
            }
        }
        if replica.version() != self.replica.version() {
            return Err(Fault::Reload("rebase did not reach the applied version"));
        }
        self.replica = replica;
        self.captured = captured;
        self.set_authoritative(snapshot);
        Ok(())
    }

    fn set_authoritative(&mut self, snapshot: StreamSnapshot) {
        self.checkpoint_to_push = Some((snapshot.version, snapshot.hash));
        self.authoritative = Some(snapshot);
        self.trim();
    }

    /// Drops the commands no `Sync`, fan-out, resume or rebase needs: those covered by the authoritative
    /// snapshot that every synced subscriber already received.
    fn trim(&mut self) {
        let keep_after = self.base_version().min(self.fanned_out);
        let cut = self.since_snapshot.partition_point(|c| c.version <= keep_after);
        self.since_snapshot.drain(..cut);
    }

    /// Whether the position can be continued from the commands the core holds.
    fn can_resume(&self, resume: &StreamResume) -> bool {
        resume.generation == self.generation
            && resume.version >= self.base_version()
            && resume.version <= self.replica.version()
    }

    /// The frames to send now: new commands to the synced subscribers, the tail to the resuming ones,
    /// a `Sync` (and the rest of a large backlog as `Commands`) to the new ones. Frames of one step are
    /// shared.
    pub fn take_frames(&mut self) -> Vec<(Uuid, StreamMessage)> {
        let (stream_id, generation) = (self.stream_id.clone(), self.generation);
        let mut frames = Vec::new();

        if self.replica.version() > self.fanned_out && !self.synced.is_empty() {
            let start = self.since_snapshot.partition_point(|c| c.version <= self.fanned_out);
            for chunk in self.since_snapshot[start..].chunks(FRAME_COMMANDS) {
                let commands: Arc<[StreamCommand]> = chunk.into();
                for connection_id in &self.synced {
                    frames.push((
                        *connection_id,
                        StreamMessage::Commands {
                            stream_id: stream_id.clone(),
                            generation,
                            commands: commands.clone(),
                        },
                    ));
                }
            }
        }

        // A resume the core cannot serve (any more) becomes a full sync.
        let resuming: Vec<(Uuid, StreamResume)> = self.pending_resume.drain().collect();
        for (connection_id, resume) in resuming {
            if self.can_resume(&resume) {
                let start = self.since_snapshot.partition_point(|c| c.version <= resume.version);
                let tail = &self.since_snapshot[start..];
                // A resume is always answered: a client at the head gets an empty list, which tells it the
                // position was accepted.
                if tail.is_empty() {
                    frames.push((
                        connection_id,
                        StreamMessage::Commands {
                            stream_id: stream_id.clone(),
                            generation,
                            commands: Arc::from(Vec::new()),
                        },
                    ));
                }
                for chunk in tail.chunks(FRAME_COMMANDS) {
                    frames.push((
                        connection_id,
                        StreamMessage::Commands {
                            stream_id: stream_id.clone(),
                            generation,
                            commands: chunk.into(),
                        },
                    ));
                }
                self.synced.insert(connection_id);
            } else {
                self.pending_sync.insert(connection_id);
            }
        }

        if !self.pending_sync.is_empty() {
            let after = self.base_version();
            let start = self.since_snapshot.partition_point(|c| c.version <= after);
            let mut chunks = self.since_snapshot[start..]
                .chunks(FRAME_COMMANDS)
                .map(Arc::<[StreamCommand]>::from);
            let first = chunks.next().unwrap_or_else(|| Arc::from(Vec::new()));
            let rest: Vec<Arc<[StreamCommand]>> = chunks.collect();
            for connection_id in self.pending_sync.drain() {
                frames.push((
                    connection_id,
                    StreamMessage::Sync {
                        stream_id: stream_id.clone(),
                        generation,
                        snapshot_every: self.kind.snapshot_every,
                        snapshot: self.authoritative.clone(),
                        commands: first.clone(),
                    },
                ));
                for commands in &rest {
                    frames.push((
                        connection_id,
                        StreamMessage::Commands {
                            stream_id: stream_id.clone(),
                            generation,
                            commands: commands.clone(),
                        },
                    ));
                }
                self.synced.insert(connection_id);
            }
        }

        self.fanned_out = self.replica.version();
        self.trim();
        frames
    }

    /// The latest resolved checkpoint for every synced subscriber, once its commands were delivered.
    pub fn take_checkpoint_frames(&mut self) -> Vec<(Uuid, StreamMessage)> {
        let Some((version, hash)) = self
            .checkpoint_to_push
            .filter(|(version, _)| *version <= self.fanned_out)
        else {
            return Vec::new();
        };
        self.checkpoint_to_push = None;
        self.synced
            .iter()
            .map(|connection_id| {
                (
                    *connection_id,
                    StreamMessage::Checkpoint {
                        stream_id: self.stream_id.clone(),
                        version,
                        hash,
                    },
                )
            })
            .collect()
    }

    /// Starts a reload: the synced subscribers get `Reset`, then everyone gets a `Sync` once loaded.
    pub fn take_reset_frames(&mut self) -> Vec<(Uuid, StreamMessage)> {
        let synced: Vec<Uuid> = self.synced.drain().collect();
        self.pending_sync.extend(synced.iter().copied());
        synced
            .into_iter()
            .map(|connection_id| {
                (
                    connection_id,
                    StreamMessage::Reset {
                        stream_id: self.stream_id.clone(),
                    },
                )
            })
            .collect()
    }
}

/// Captures the replica at a checkpoint version. An encoding failure (a bug of the state type) only
/// skips the checkpoint: snapshots are an optimization and a drift check.
fn capture<S: ReplayState>(stream_id: &StreamId, kind: &KindSettings, replica: &Replica<S>) -> Option<Captured> {
    let version = replica.version();
    if !kind.is_snapshot_version(version) {
        return None;
    }
    match encode_snapshot(replica.state()) {
        Ok(data) => Some(Captured {
            version,
            hash: replica.hash(),
            data: data.into(),
        }),
        Err(err) => {
            tracing::error!("[{stream_id}] Failed to encode the state at version {version}: {err}");
            None
        }
    }
}

fn push_captured(captured: &mut VecDeque<Captured>, checkpoint: Captured) {
    if captured.len() == MAX_CAPTURED {
        captured.pop_front();
    }
    captured.push_back(checkpoint);
}

#[cfg(test)]
#[path = "stream_core_test.rs"]
mod stream_core_test;
