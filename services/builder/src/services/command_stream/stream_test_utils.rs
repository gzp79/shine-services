//! What the tests of the workers and of the registry share: a sink that records what is sent to each
//! connection, helpers to read it, and a second kind of stream next to the chat room.

use crate::repositories::command_log::StreamId;
use crate::services::command_stream::{StreamEgress, StreamMessage, WorkerConfig, WorkerHandle};
use serde::{Deserialize, Serialize};
use shine_game::replica::{CommandHeader, ReplayState, StableHash, StableHasher};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

pub(crate) const WAIT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct EgressInner {
    received: HashMap<Uuid, Vec<Arc<StreamMessage>>>,
    dead: HashSet<Uuid>,
}

/// Records what is sent to each connection; a connection can be made dead.
#[derive(Clone, Default)]
pub(crate) struct TestEgress {
    inner: Arc<Mutex<EgressInner>>,
}

impl StreamEgress for TestEgress {
    async fn send(&self, connection_id: Uuid, message: Arc<StreamMessage>) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if inner.dead.contains(&connection_id) {
            return false;
        }
        inner.received.entry(connection_id).or_default().push(message);
        true
    }
}

impl TestEgress {
    pub(crate) fn messages(&self, connection_id: Uuid) -> Vec<Arc<StreamMessage>> {
        self.inner
            .lock()
            .unwrap()
            .received
            .get(&connection_id)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn kill(&self, connection_id: Uuid) {
        self.inner.lock().unwrap().dead.insert(connection_id);
    }

    pub(crate) fn revive(&self, connection_id: Uuid) {
        self.inner.lock().unwrap().dead.remove(&connection_id);
    }

    /// Waits until the messages of the connection satisfy `condition`.
    pub(crate) async fn wait(&self, connection_id: Uuid, condition: impl Fn(&[Arc<StreamMessage>]) -> bool) {
        let result = timeout(WAIT, async {
            loop {
                if condition(&self.messages(connection_id)) {
                    return;
                }
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(result.is_ok(), "timed out; got {:#?}", self.messages(connection_id));
    }
}

/// The versions of the commands a connection was sent for a stream, in order, from syncs and command
/// messages.
pub(crate) fn versions_of(messages: &[Arc<StreamMessage>], stream: &StreamId) -> Vec<i64> {
    let mut versions = Vec::new();
    for message in messages.iter().filter(|m| m.stream_id() == stream) {
        match message.as_ref() {
            StreamMessage::Sync { commands, .. } | StreamMessage::Commands { commands, .. } => {
                versions.extend(commands.iter().map(|c| c.version));
            }
            _ => {}
        }
    }
    versions
}

pub(crate) fn kinds(messages: &[Arc<StreamMessage>]) -> Vec<String> {
    messages
        .iter()
        .map(|m| match m.as_ref() {
            StreamMessage::Sync { commands, .. } => format!("sync:{}", commands.len()),
            StreamMessage::Commands { commands, .. } => format!("commands:{}", commands.len()),
            StreamMessage::Checkpoint { version, .. } => format!("checkpoint:{version}"),
            StreamMessage::Reset { .. } => "reset".to_string(),
            StreamMessage::Unsynced { .. } => "unsynced".to_string(),
        })
        .collect()
}

/// A worker that does only what it is told: no backstop tick, no wake spacing and no unloading.
pub(crate) fn quiet() -> WorkerConfig {
    WorkerConfig {
        backstop: Duration::from_secs(3600),
        min_wake_spacing: Duration::ZERO,
        unload_grace: Duration::from_secs(3600),
        max_failures: 3,
    }
}

pub(crate) async fn wait_finished(worker: &WorkerHandle) {
    let result = timeout(WAIT, async {
        while !worker.is_finished() {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(result.is_ok(), "the worker did not end");
}

/// A second kind of stream: a running total.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CounterState {
    pub total: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum CounterCommand {
    Add { amount: i64 },
}

impl StableHash for CounterState {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.total.stable_hash(hasher);
    }
}

impl StableHash for CounterCommand {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        match self {
            CounterCommand::Add { amount } => {
                0u8.stable_hash(hasher);
                amount.stable_hash(hasher);
            }
        }
    }
}

impl ReplayState for CounterState {
    type Command = CounterCommand;

    const KIND: &'static str = "counter-test";

    fn apply(&mut self, _header: &CommandHeader, command: &Self::Command) {
        match command {
            CounterCommand::Add { amount } => self.total = self.total.saturating_add(*amount),
        }
    }

    fn is_well_formed(command: &Self::Command) -> bool {
        match command {
            CounterCommand::Add { amount } => amount.abs() <= 1000,
        }
    }
}
