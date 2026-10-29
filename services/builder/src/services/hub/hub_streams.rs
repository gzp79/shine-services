use crate::repositories::command_log::StreamId;
use std::{collections::HashMap, collections::HashSet, sync::Mutex};
use uuid::Uuid;

/// The streams each connection is subscribed to. It is changed when a transport subscribes or
/// unsubscribes, so what a connection does next sees it, and cleared when the connection is gone.
#[derive(Default)]
pub struct StreamSubscriptions {
    by_connection: Mutex<HashMap<Uuid, HashSet<StreamId>>>,
}

impl StreamSubscriptions {
    pub fn subscribe(&self, connection_id: Uuid, stream_id: StreamId) {
        self.by_connection
            .lock()
            .unwrap()
            .entry(connection_id)
            .or_default()
            .insert(stream_id);
    }

    pub fn unsubscribe(&self, connection_id: Uuid, stream_id: &StreamId) {
        let mut by_connection = self.by_connection.lock().unwrap();
        if let Some(streams) = by_connection.get_mut(&connection_id) {
            streams.remove(stream_id);
            if streams.is_empty() {
                by_connection.remove(&connection_id);
            }
        }
    }

    pub fn contains(&self, connection_id: Uuid, stream_id: &StreamId) -> bool {
        self.by_connection
            .lock()
            .unwrap()
            .get(&connection_id)
            .is_some_and(|streams| streams.contains(stream_id))
    }

    pub fn remove_connection(&self, connection_id: Uuid) {
        self.by_connection.lock().unwrap().remove(&connection_id);
    }

    pub fn clear(&self) {
        self.by_connection.lock().unwrap().clear();
    }
}
