//! What the tests of the workers and of the registry share: a sink that records what is sent to each
//! connection and helpers to read it.

use crate::{models::messages::StreamMessage, services::StreamEgress};
use shine_game::replica::StreamId;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct EgressInner {
    received: HashMap<Uuid, Vec<Arc<StreamMessage>>>,
    dead: HashSet<Uuid>,
}

/// Records what is sent to each connection; a connection can be made dead.
#[derive(Clone, Default)]
pub struct TestEgress {
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
    pub fn messages(&self, connection_id: Uuid) -> Vec<Arc<StreamMessage>> {
        self.inner
            .lock()
            .unwrap()
            .received
            .get(&connection_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn kill(&self, connection_id: Uuid) {
        self.inner.lock().unwrap().dead.insert(connection_id);
    }

    pub fn revive(&self, connection_id: Uuid) {
        self.inner.lock().unwrap().dead.remove(&connection_id);
    }

    /// Waits until the messages of the connection satisfy `condition`.
    pub async fn wait(&self, connection_id: Uuid, condition: impl Fn(&[Arc<StreamMessage>]) -> bool) {
        let result = timeout(WAIT_TIMEOUT, async {
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

    /// The kind of each message, as `kind` or `kind:count`.
    pub fn kinds(messages: &[Arc<StreamMessage>]) -> Vec<String> {
        messages
            .iter()
            .map(|m| match m.as_ref() {
                StreamMessage::Sync { commands, .. } => format!("sync:{}", commands.len()),
                StreamMessage::Commands { commands, .. } => format!("commands:{}", commands.len()),
                StreamMessage::Checkpoint { version, .. } => format!("checkpoint:{version}"),
                StreamMessage::Unsynced { .. } => "unsynced".to_string(),
                StreamMessage::CommandFailed { .. } => "command-failed".to_string(),
                StreamMessage::SubscribeFailed { .. } => "subscribe-failed".to_string(),
            })
            .collect()
    }

    /// The versions of the commands of a stream, in the order they appear in sync and command messages.
    pub fn versions(messages: &[Arc<StreamMessage>], stream_id: &StreamId) -> Vec<i64> {
        let mut versions = Vec::new();
        for message in messages.iter().filter(|m| m.stream_id() == stream_id) {
            match message.as_ref() {
                StreamMessage::Sync { commands, .. } | StreamMessage::Commands { commands, .. } => {
                    versions.extend(commands.iter().map(|c| c.version));
                }
                _ => {}
            }
        }
        versions
    }
}
