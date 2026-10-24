use crate::repositories::command_log::StreamId;
use std::{collections::HashMap, sync::Mutex};
use uuid::Uuid;

/// The streams each connection is subscribed to, and their kinds: what the hub needs to tell whether a command
/// of a connection is for a stream it is in. It is changed when a transport subscribes or unsubscribes, so
/// what a connection does next sees it, and cleared when the connection is gone.
#[derive(Default)]
pub struct StreamSubscriptions {
    by_connection: Mutex<HashMap<Uuid, HashMap<StreamId, String>>>,
}

impl StreamSubscriptions {
    pub fn subscribe(&self, connection_id: Uuid, stream_id: StreamId, kind: String) {
        self.by_connection
            .lock()
            .unwrap()
            .entry(connection_id)
            .or_default()
            .insert(stream_id, kind);
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

    /// The kind of the stream, if the connection is subscribed to it.
    pub fn kind_of(&self, connection_id: Uuid, stream_id: &StreamId) -> Option<String> {
        self.by_connection
            .lock()
            .unwrap()
            .get(&connection_id)
            .and_then(|streams| streams.get(stream_id))
            .cloned()
    }

    pub fn remove_connection(&self, connection_id: Uuid) {
        self.by_connection.lock().unwrap().remove(&connection_id);
    }

    pub fn clear(&self) {
        self.by_connection.lock().unwrap().clear();
    }
}
