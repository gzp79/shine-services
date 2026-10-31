//! What the tests of the workers and of the registry share: a sink that records what is sent to each
//! connection and helpers to read it.

use crate::{
    models::messages::StreamMessage, services::StreamEgress, testing::command_stream::utils::WAIT_NOTIFICATION,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

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
        let result = timeout(WAIT_NOTIFICATION, async {
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
