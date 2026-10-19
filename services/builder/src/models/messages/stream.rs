use crate::models::messages::{ToTopic, TopicKey};
use shine_infra::db::command_log::StreamId;
use std::sync::Arc;
use uuid::Uuid;

/// A command at its version; `data` is the canonical command JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamCommand {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub data: Arc<str>,
}

/// A stored snapshot (the encoded state at a version).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamSnapshot {
    pub version: i64,
    pub hash: u64,
    pub data: Arc<[u8]>,
}

/// Where a client wants to continue a stream from: the epoch of the history it follows and the last
/// version it applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamResume {
    pub generation: i64,
    pub version: i64,
}

/// Egress to one connection about one stream. Sent directly, never broadcast; the command lists are
/// shared by every subscriber of a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamMessage {
    /// The state to start from (a snapshot, if any) and the first commands after it; more follow as
    /// `Commands`. It replaces the client's state of the stream.
    Sync {
        stream_id: StreamId,
        generation: i64,
        /// The snapshot cadence of the stream: a client hashes its state at those versions to check
        /// the checkpoints.
        snapshot_every: i64,
        snapshot: Option<StreamSnapshot>,
        commands: Arc<[StreamCommand]>,
    },
    Commands {
        stream_id: StreamId,
        generation: i64,
        commands: Arc<[StreamCommand]>,
    },
    Checkpoint {
        stream_id: StreamId,
        version: i64,
        hash: u64,
    },
    /// The client's view was rolled back; a fresh `Sync` follows.
    Reset {
        stream_id: StreamId,
    },
    Nack {
        stream_id: StreamId,
        command_id: Uuid,
        reason: &'static str,
    },
    SubscribeFailed {
        stream_id: StreamId,
        reason: &'static str,
    },
}

/// Subscription changes, published by the hub's command loop to internal subscribers only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamEvent {
    Subscribed {
        connection_id: Uuid,
        stream_id: StreamId,
        kind: &'static str,
        resume: Option<StreamResume>,
    },
    Unsubscribed {
        connection_id: Uuid,
        stream_id: StreamId,
    },
}

impl ToTopic for StreamEvent {
    fn topic(&self) -> TopicKey {
        TopicKey::Stream
    }
}
