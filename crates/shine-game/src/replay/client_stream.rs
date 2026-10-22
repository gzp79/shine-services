//! The client side of one command stream: a pure state machine that turns the server frames into the
//! replayed state and the frames to send back. It owns no socket, timer or random source: the caller feeds it
//! the socket events and the frames, sends what it returns, and supplies the command ids and the timeouts.

use crate::replay::{
    decode_command, decode_snapshot, encode_command,
    protocol::{decode_snapshot_data, hash_from_hex, ClientFrame, ServerFrame, WireCommand, WireResume, WireSnapshot},
    stable_hash_of, ApplyOutcome, CommandContext, CommandEnvelope, ReplayState, Replica,
};
use std::collections::VecDeque;
use uuid::Uuid;

/// Hashes kept to compare a checkpoint that arrives after later commands.
const KEPT_HASHES: usize = 8;

/// What the user interface can show about the freshness of the view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    /// The socket is being opened.
    Connecting,
    /// Subscribed, waiting for the first answer.
    Syncing,
    /// The view follows the stream.
    Live,
    /// The view may be stale until the next `streamSync`.
    Resyncing,
    /// No socket; the view is what was last known.
    Offline,
    /// The stream cannot be followed (the reason); a new connection tries again.
    Failed(String),
}

/// What a frame meant, for the caller (a notification, a hint in the interface).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientEvent {
    /// Nothing to report (another stream, a matching checkpoint, a stale frame).
    None,
    /// The state was replaced or resumed and the stream is live.
    Synced,
    /// New commands were applied.
    Applied {
        commands: usize,
    },
    /// The server rolled the stream back; a fresh `streamSync` follows.
    Reset,
    /// The client asked for a fresh `streamSync` itself.
    ResyncRequested(&'static str),
    /// A write was refused; `dropped` when the command left the pending list.
    Nacked {
        command_id: Uuid,
        reason: String,
        dropped: bool,
    },
    Failed(String),
}

/// The result of one server frame.
#[derive(Debug, PartialEq, Eq)]
pub struct FrameOutcome {
    /// Frames to send to the server, in order.
    pub send: Vec<ClientFrame>,
    pub event: ClientEvent,
}

impl FrameOutcome {
    fn nothing() -> Self {
        Self {
            send: vec![],
            event: ClientEvent::None,
        }
    }

    fn event(event: ClientEvent) -> Self {
        Self { send: vec![], event }
    }
}

/// Why a wire command could not be applied.
enum ApplyFault {
    Undecodable,
    Gap,
}

/// One followed stream: the state the server agrees on, the writes it has not confirmed yet, and the view
/// that shows both.
pub struct ClientStream<S: ReplayState> {
    stream_id: String,
    user_id: Uuid,
    status: SyncStatus,
    /// The epoch of the history the state follows; `None` until the first sync.
    generation: Option<i64>,
    snapshot_every: i64,
    authoritative: Replica<S>,
    /// Writes not seen in the log yet, in submit order.
    pending: Vec<(Uuid, S::Command)>,
    /// The authoritative state with the pending writes applied.
    local: S,
    /// Hashes of the authoritative state at the multiples of `snapshot_every`, oldest first.
    hashes: VecDeque<(i64, u64)>,
}

impl<S: ReplayState> ClientStream<S> {
    /// A stream followed by `user_id`, whose writes carry that author.
    pub fn new(stream_id: impl Into<String>, user_id: Uuid) -> Self {
        Self {
            stream_id: stream_id.into(),
            user_id,
            status: SyncStatus::Connecting,
            generation: None,
            snapshot_every: 0,
            authoritative: Replica::new(),
            pending: Vec::new(),
            local: S::default(),
            hashes: VecDeque::new(),
        }
    }

    pub fn stream_id(&self) -> &str {
        &self.stream_id
    }

    pub fn status(&self) -> &SyncStatus {
        &self.status
    }

    /// What to show: the confirmed state with the pending writes on top.
    pub fn local(&self) -> &S {
        &self.local
    }

    /// The state the server agrees on.
    pub fn authoritative(&self) -> &Replica<S> {
        &self.authoritative
    }

    pub fn pending(&self) -> &[(Uuid, S::Command)] {
        &self.pending
    }

    /// The socket is open: subscribes, continuing from the known position when there is one.
    pub fn on_connected(&mut self) -> Vec<ClientFrame> {
        self.status = SyncStatus::Syncing;
        vec![self.subscribe_frame(true)]
    }

    /// The socket closed. The view stays as it was; pending writes wait for the next sync.
    pub fn on_disconnected(&mut self) {
        self.status = SyncStatus::Offline;
    }

    /// The answer to a subscribe did not come in time: close the socket and reconnect.
    pub fn on_sync_timeout(&mut self) {
        if matches!(self.status, SyncStatus::Syncing | SyncStatus::Resyncing) {
            self.status = SyncStatus::Offline;
        }
    }

    /// Records a write and shows it at once. The frame to send is returned only while the stream is live;
    /// otherwise the write goes out after the next sync. A command that cannot be encoded is not recorded.
    pub fn submit(&mut self, command_id: Uuid, command: S::Command) -> Vec<ClientFrame> {
        let Ok(text) = encode_command::<S>(&command) else {
            return Vec::new();
        };
        let version = self.authoritative.version() + self.pending.len() as i64 + 1;
        self.local.apply(
            &CommandContext {
                version,
                command_id,
                user_id: self.user_id,
            },
            &command,
        );
        self.pending.push((command_id, command));
        if self.status == SyncStatus::Live {
            vec![self.command_frame(command_id, text)]
        } else {
            Vec::new()
        }
    }

    /// Sends the pending writes again (after a nack that keeps them, once the caller waited). The server
    /// accepts a command id once, so a write that landed meanwhile answers `duplicate` and is dropped.
    pub fn retry_pending(&mut self) -> Vec<ClientFrame> {
        if self.status == SyncStatus::Live {
            self.pending_frames()
        } else {
            Vec::new()
        }
    }

    /// Feeds one frame of the server.
    pub fn on_frame(&mut self, frame: ServerFrame) -> FrameOutcome {
        if frame_stream_id(&frame) != self.stream_id {
            return FrameOutcome::nothing();
        }
        match frame {
            ServerFrame::StreamSync {
                generation,
                snapshot_every,
                snapshot,
                commands,
                ..
            } => self.on_sync(generation, snapshot_every, snapshot, &commands),
            ServerFrame::StreamCommands { generation, commands, .. } => self.on_commands(generation, &commands),
            ServerFrame::StreamCheckpoint { version, hash, .. } => self.on_checkpoint(version, &hash),
            ServerFrame::StreamReset { .. } => {
                self.status = SyncStatus::Resyncing;
                FrameOutcome::event(ClientEvent::Reset)
            }
            ServerFrame::StreamNack { command_id, reason, .. } => self.on_nack(command_id, reason),
            ServerFrame::StreamSubscribeFailed { reason, .. } => {
                self.status = SyncStatus::Failed(reason.clone());
                FrameOutcome::event(ClientEvent::Failed(reason))
            }
        }
    }

    fn on_sync(
        &mut self,
        generation: i64,
        snapshot_every: i64,
        snapshot: Option<WireSnapshot>,
        commands: &[WireCommand],
    ) -> FrameOutcome {
        let mut replica = Replica::new();
        let mut hashes = VecDeque::new();
        if let Some(snapshot) = snapshot {
            let Some(state) = decode_snapshot_data(&snapshot.data).and_then(|bytes| decode_snapshot::<S>(&bytes).ok())
            else {
                return self.fail("the snapshot cannot be decoded");
            };
            let replica_from_snapshot = Replica::from_snapshot(snapshot.version, state);
            if hash_from_hex(&snapshot.hash) != Some(replica_from_snapshot.hash()) {
                return self.fail("the snapshot does not match its hash");
            }
            keep_hash(&mut hashes, snapshot_every, &replica_from_snapshot);
            replica = replica_from_snapshot;
        }
        for command in commands {
            match apply_wire(&mut replica, command) {
                Ok(true) => keep_hash(&mut hashes, snapshot_every, &replica),
                Ok(false) => {}
                Err(ApplyFault::Undecodable) => return self.fail("a command cannot be decoded"),
                Err(ApplyFault::Gap) => return self.fail("the sync is not contiguous"),
            }
        }

        // Writes the log already holds are confirmed; the others go out again with their ids.
        self.pending
            .retain(|(id, _)| !commands.iter().any(|command| command.command_id == *id));
        self.authoritative = replica;
        self.generation = Some(generation);
        self.snapshot_every = snapshot_every;
        self.hashes = hashes;
        self.status = SyncStatus::Live;
        self.rebuild_local();
        FrameOutcome {
            send: self.pending_frames(),
            event: ClientEvent::Synced,
        }
    }

    fn on_commands(&mut self, generation: i64, commands: &[WireCommand]) -> FrameOutcome {
        // Before the first sync there is no state to continue; while resyncing the answer is a sync.
        if !matches!(self.status, SyncStatus::Live | SyncStatus::Syncing) || self.generation.is_none() {
            return FrameOutcome::nothing();
        }
        if self.generation != Some(generation) {
            return self.resync("the history changed");
        }
        let mut applied = 0;
        for command in commands {
            match apply_wire(&mut self.authoritative, command) {
                Ok(true) => {
                    applied += 1;
                    self.pending.retain(|(id, _)| *id != command.command_id);
                    keep_hash(&mut self.hashes, self.snapshot_every, &self.authoritative);
                }
                Ok(false) => {}
                Err(ApplyFault::Undecodable) => return self.fail("a command cannot be decoded"),
                Err(ApplyFault::Gap) => {
                    self.rebuild_local();
                    return self.resync("a command is missing");
                }
            }
        }
        self.rebuild_local();
        if self.status == SyncStatus::Syncing {
            // The answer to a resume: the position is valid, the writes made meanwhile go out again.
            self.status = SyncStatus::Live;
            return FrameOutcome {
                send: self.pending_frames(),
                event: ClientEvent::Synced,
            };
        }
        FrameOutcome::event(ClientEvent::Applied { commands: applied })
    }

    fn on_checkpoint(&mut self, version: i64, hash: &str) -> FrameOutcome {
        if self.status != SyncStatus::Live {
            return FrameOutcome::nothing();
        }
        let Some((_, own)) = self.hashes.iter().find(|(v, _)| *v == version) else {
            // Older than the hashes kept, or not a version this client hashed.
            return FrameOutcome::nothing();
        };
        if hash_from_hex(hash) == Some(*own) {
            FrameOutcome::nothing()
        } else {
            self.resync("the state differs from the checkpoint")
        }
    }

    fn on_nack(&mut self, command_id: Uuid, reason: String) -> FrameOutcome {
        if !self.pending.iter().any(|(id, _)| *id == command_id) {
            return FrameOutcome::nothing();
        }
        match reason.as_str() {
            // Transient: the write stays pending and is sent again.
            "failed" | "busy" | "notReady" => FrameOutcome::event(ClientEvent::Nacked {
                command_id,
                reason,
                dropped: false,
            }),
            // The server does not follow this connection for the stream: subscribe again.
            "notSubscribed" => {
                let mut outcome = self.resync("not subscribed");
                outcome.event = ClientEvent::Nacked {
                    command_id,
                    reason,
                    dropped: false,
                };
                outcome
            }
            // Final: already in the log, or refused for good.
            _ => {
                self.pending.retain(|(id, _)| *id != command_id);
                self.rebuild_local();
                FrameOutcome::event(ClientEvent::Nacked {
                    command_id,
                    reason,
                    dropped: true,
                })
            }
        }
    }

    /// Asks for a fresh `streamSync` from scratch.
    fn resync(&mut self, reason: &'static str) -> FrameOutcome {
        self.status = SyncStatus::Resyncing;
        FrameOutcome {
            send: vec![self.subscribe_frame(false)],
            event: ClientEvent::ResyncRequested(reason),
        }
    }

    fn fail(&mut self, reason: &str) -> FrameOutcome {
        self.status = SyncStatus::Failed(reason.to_string());
        FrameOutcome::event(ClientEvent::Failed(reason.to_string()))
    }

    fn subscribe_frame(&self, resume: bool) -> ClientFrame {
        ClientFrame::StreamSubscribe {
            stream_id: self.stream_id.clone(),
            kind: S::KIND.to_string(),
            resume: self.generation.filter(|_| resume).map(|generation| WireResume {
                generation,
                version: self.authoritative.version(),
            }),
        }
    }

    fn command_frame(&self, command_id: Uuid, command: String) -> ClientFrame {
        ClientFrame::StreamCommand {
            stream_id: self.stream_id.clone(),
            command_id,
            command,
        }
    }

    fn pending_frames(&self) -> Vec<ClientFrame> {
        self.pending
            .iter()
            .filter_map(|(id, command)| {
                encode_command::<S>(command)
                    .ok()
                    .map(|text| self.command_frame(*id, text))
            })
            .collect()
    }

    fn rebuild_local(&mut self) {
        let mut local = self.authoritative.state().clone();
        for (index, (command_id, command)) in self.pending.iter().enumerate() {
            let context = CommandContext {
                version: self.authoritative.version() + index as i64 + 1,
                command_id: *command_id,
                user_id: self.user_id,
            };
            local.apply(&context, command);
        }
        self.local = local;
    }
}

fn frame_stream_id(frame: &ServerFrame) -> &str {
    match frame {
        ServerFrame::StreamSync { stream_id, .. }
        | ServerFrame::StreamCommands { stream_id, .. }
        | ServerFrame::StreamCheckpoint { stream_id, .. }
        | ServerFrame::StreamReset { stream_id }
        | ServerFrame::StreamNack { stream_id, .. }
        | ServerFrame::StreamSubscribeFailed { stream_id, .. } => stream_id,
    }
}

/// Applies a command of the wire; `true` when it was new.
fn apply_wire<S: ReplayState>(replica: &mut Replica<S>, wire: &WireCommand) -> Result<bool, ApplyFault> {
    let command = decode_command::<S>(&wire.command).map_err(|_| ApplyFault::Undecodable)?;
    let envelope = CommandEnvelope {
        version: wire.version,
        command_id: wire.command_id,
        user_id: wire.user_id,
        command,
    };
    match replica.apply_next(&envelope) {
        Ok(ApplyOutcome::Applied) => Ok(true),
        Ok(ApplyOutcome::AlreadyApplied) => Ok(false),
        Err(_) => Err(ApplyFault::Gap),
    }
}

/// Remembers the hash of the replica when its version is a multiple of `snapshot_every`.
fn keep_hash<S: ReplayState>(hashes: &mut VecDeque<(i64, u64)>, snapshot_every: i64, replica: &Replica<S>) {
    if snapshot_every > 0 && replica.version() > 0 && replica.version() % snapshot_every == 0 {
        if hashes.len() == KEPT_HASHES {
            hashes.pop_front();
        }
        hashes.push_back((replica.version(), stable_hash_of(replica.state())));
    }
}
