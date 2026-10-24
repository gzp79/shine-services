//! The client side of a command stream: the replayed state, the commands not confirmed yet, and the
//! reactions to the frames of the server. It does no I/O; the caller moves the frames.

use crate::replica::{
    decode_snapshot, parse_server_message, ClientMessage, CommandEnvelope, CommandHeader, ReplayState, Replica,
    ServerMessage, StreamId, WireCommand,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::VecDeque;
use uuid::Uuid;

/// How many applied versions keep their state hash, to check the checkpoints of the server against.
const HASH_HISTORY: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamStatus {
    /// Subscribed or about to be, no state from the server yet (or it asked for a new one).
    Waiting,
    /// The state follows the server.
    Live,
    /// The server cannot serve the stream or its state cannot be trusted; subscribe again later.
    Unsynced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SubmitError {
    #[error("the stream is not live")]
    NotLive,
}

struct Pending<C> {
    command_id: Uuid,
    command: C,
}

/// The state of a stream as one client sees it: the confirmed `Replica` of the server's log, plus the
/// commands of this client that the log does not hold yet. `view` shows both.
pub struct ClientStream<S: ReplayState> {
    stream_id: StreamId,
    user_id: Uuid,
    replica: Replica<S>,
    pending: Vec<Pending<S::Command>>,
    /// `(version, state hash)` of the latest applied versions.
    hashes: VecDeque<(i64, u64)>,
    status: StreamStatus,
    revision: u32,
}

fn parse_hash(text: &str) -> Option<u64> {
    u64::from_str_radix(text, 16).ok()
}

impl<S: ReplayState> ClientStream<S> {
    pub fn new(stream_id: StreamId, user_id: Uuid) -> Self {
        Self {
            stream_id,
            user_id,
            replica: Replica::new(),
            pending: Vec::new(),
            hashes: VecDeque::new(),
            status: StreamStatus::Waiting,
            revision: 0,
        }
    }

    pub fn status(&self) -> StreamStatus {
        self.status
    }

    /// The version of the confirmed state.
    pub fn version(&self) -> i64 {
        self.replica.version()
    }

    /// The state the server confirmed.
    pub fn confirmed(&self) -> &S {
        self.replica.state()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// The commands of this client that the server has not confirmed, oldest first.
    pub fn pending_commands(&self) -> impl Iterator<Item = &S::Command> {
        self.pending.iter().map(|pending| &pending.command)
    }

    /// Changes whenever what `view` shows changes. Status changes do not count.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// The confirmed state with the pending commands applied on top.
    pub fn view(&self) -> S {
        let mut state = self.replica.state().clone();
        for (offset, pending) in self.pending.iter().enumerate() {
            let header = CommandHeader {
                version: self.replica.version() + 1 + offset as i64,
                command_id: pending.command_id,
                user_id: self.user_id,
                chain_hash: 0,
            };
            state.apply(&header, &pending.command);
        }
        state
    }

    /// The frame that subscribes to the stream, or asks for it to be synced again.
    pub fn subscribe_frame(&self) -> String {
        frame(&ClientMessage::StreamSubscribe {
            stream_id: self.stream_id.to_string(),
            kind: S::KIND.to_string(),
        })
    }

    /// Asks the server for the state again, as after a connect: waits for the sync (the view stays), and
    /// the pending commands are sent again with it. Returns the frame to send.
    pub fn resync(&mut self) -> String {
        self.status = StreamStatus::Waiting;
        self.subscribe_frame()
    }

    /// Takes a command of this client: it shows in the view at once and is pending until the log holds
    /// it. Returns the frame to send. `command_id` is the client's own and must be unique.
    pub fn submit(&mut self, command_id: Uuid, command: S::Command) -> Result<String, SubmitError> {
        if self.status != StreamStatus::Live {
            return Err(SubmitError::NotLive);
        }
        let message = command_frame(&self.stream_id, command_id, &command);
        self.pending.push(Pending { command_id, command });
        self.revision = self.revision.wrapping_add(1);
        Ok(message)
    }

    /// Reacts to a frame of the server and returns the frames to send back. Frames that are not
    /// understood or belong to another stream are ignored.
    pub fn handle_message(&mut self, text: &str) -> Vec<String> {
        let Some(message) = parse_server_message(text) else {
            return Vec::new();
        };
        match message {
            ServerMessage::StreamSync { stream_id, snapshot, commands } if self.is_mine(&stream_id) => {
                self.on_sync(snapshot, commands)
            }
            ServerMessage::StreamCommands { stream_id, commands } if self.is_mine(&stream_id) => {
                self.on_commands(commands)
            }
            ServerMessage::StreamCheckpoint { stream_id, version, state_hash } if self.is_mine(&stream_id) => {
                self.on_checkpoint(version, &state_hash)
            }
            ServerMessage::StreamReset { stream_id } if self.is_mine(&stream_id) => {
                self.status = StreamStatus::Waiting;
                Vec::new()
            }
            ServerMessage::StreamUnsynced { stream_id } | ServerMessage::StreamSubscribeFailed { stream_id, .. }
                if self.is_mine(&stream_id) =>
            {
                self.status = StreamStatus::Unsynced;
                Vec::new()
            }
            ServerMessage::StreamNack { stream_id, command_id, reason } if self.is_mine(&stream_id) => {
                self.on_nack(command_id, &reason)
            }
            _ => Vec::new(),
        }
    }

    fn is_mine(&self, stream_id: &str) -> bool {
        self.stream_id.as_str() == stream_id
    }

    /// The server's state cannot be followed: ask for a new one.
    fn resubscribe(&mut self) -> Vec<String> {
        self.status = StreamStatus::Waiting;
        vec![self.subscribe_frame()]
    }

    fn on_sync(&mut self, snapshot: Option<crate::replica::WireSnapshot>, commands: Vec<WireCommand>) -> Vec<String> {
        let mut replica = Replica::new();
        if let Some(snapshot) = snapshot {
            let Some(restored) = restore::<S>(&snapshot) else {
                self.status = StreamStatus::Unsynced;
                return Vec::new();
            };
            replica = restored;
        }
        let mut hashes = VecDeque::new();
        hashes.push_back((replica.version(), replica.hash()));
        for wire in &commands {
            if !apply_wire(&mut replica, &mut hashes, wire) {
                self.status = StreamStatus::Unsynced;
                return Vec::new();
            }
        }

        self.replica = replica;
        self.hashes = hashes;
        self.pending
            .retain(|pending| !commands.iter().any(|wire| wire.command_id == pending.command_id));
        self.status = StreamStatus::Live;
        self.revision = self.revision.wrapping_add(1);
        self.pending
            .iter()
            .map(|pending| command_frame(&self.stream_id, pending.command_id, &pending.command))
            .collect()
    }

    fn on_commands(&mut self, commands: Vec<WireCommand>) -> Vec<String> {
        if self.status != StreamStatus::Live {
            return Vec::new();
        }
        if commands
            .first()
            .is_some_and(|first| first.version > self.replica.version() + 1)
        {
            return self.resubscribe();
        }
        let mut changed = false;
        for wire in &commands {
            if wire.version <= self.replica.version() {
                continue;
            }
            if !apply_wire(&mut self.replica, &mut self.hashes, wire) {
                return self.resubscribe();
            }
            self.pending.retain(|pending| pending.command_id != wire.command_id);
            changed = true;
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
        Vec::new()
    }

    fn on_checkpoint(&mut self, version: i64, state_hash: &str) -> Vec<String> {
        if self.status != StreamStatus::Live {
            return Vec::new();
        }
        let ours = self
            .hashes
            .iter()
            .find(|(known, _)| *known == version)
            .map(|(_, hash)| *hash);
        match (ours, parse_hash(state_hash)) {
            (Some(ours), Some(theirs)) if ours != theirs => {
                self.revision = self.revision.wrapping_add(1);
                self.resubscribe()
            }
            _ => Vec::new(),
        }
    }

    fn on_nack(&mut self, command_id: Uuid, reason: &str) -> Vec<String> {
        let before = self.pending.len();
        self.pending.retain(|pending| pending.command_id != command_id);
        if self.pending.len() != before {
            self.revision = self.revision.wrapping_add(1);
        }
        // The server lost the subscription: the next sync brings the state back.
        if reason == "notSubscribed" {
            return self.resubscribe();
        }
        Vec::new()
    }
}

fn frame(message: &ClientMessage) -> String {
    serde_json::to_string(message).expect("a client message is always valid JSON")
}

fn command_frame<C: serde::Serialize>(stream_id: &StreamId, command_id: Uuid, command: &C) -> String {
    frame(&ClientMessage::StreamCommand {
        stream_id: stream_id.to_string(),
        command_id,
        command: serde_json::to_value(command).expect("a command is always valid JSON"),
    })
}

/// The replica a snapshot describes, if the snapshot is sound and its state matches its hash.
fn restore<S: ReplayState>(snapshot: &crate::replica::WireSnapshot) -> Option<Replica<S>> {
    let data = STANDARD.decode(&snapshot.data).ok()?;
    let state: S = decode_snapshot(&data).ok()?;
    let replica = Replica::from_snapshot(snapshot.version, state, parse_hash(&snapshot.chain_hash)?);
    (replica.hash() == parse_hash(&snapshot.state_hash)?).then_some(replica)
}

/// Applies a command of the log to the replica; false if it cannot be applied.
fn apply_wire<S: ReplayState>(replica: &mut Replica<S>, hashes: &mut VecDeque<(i64, u64)>, wire: &WireCommand) -> bool {
    let Ok(command) = serde_json::from_value::<S::Command>(wire.command.clone()) else {
        return false;
    };
    // The wire carries no chain hash: the replica continues its own chain, the state hashes of the
    // checkpoints are what is compared with the server.
    let envelope = CommandEnvelope::new(
        wire.version,
        wire.command_id,
        wire.user_id,
        replica.chain_hash(),
        command,
    );
    if replica.apply_next(&envelope).is_err() {
        return false;
    }
    hashes.push_back((wire.version, replica.hash()));
    if hashes.len() > HASH_HISTORY {
        hashes.pop_front();
    }
    true
}
