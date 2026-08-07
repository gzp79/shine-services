use super::connection_tracker::{spawn_connection_loop, ConnectionConsumer, ConnectionTracker};
use crate::{
    models::messages::{ChatBatch, ChatComment, HubMessage},
    repositories::chat_comments::StoredChatComment,
    services::{ChatService, HubService},
};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Max entries read from the stream in a single tick. Bounds the per-tick query and fan-out; a
/// connection further behind than this catches up over successive ticks.
const BATCH_LIMIT: usize = 128;

/// The initial cursor for a freshly seen connection: `list_after` treats it exclusively, so `"0"`
/// yields all currently retained history.
const INITIAL_CURSOR: &str = "0";

/// Periodic consumer that pushes chat to connected users.
pub struct ChatDispatcher {
    hub_service: HubService,
    chat_service: ChatService,
    cursors: HashMap<Uuid, String>,
}

impl ChatDispatcher {
    /// Starts the chat dispatcher on its own connection loop, reading the room stream once per tick
    /// and delivering per-connection, id-targeted chat batches.
    pub async fn start(service: HubService, chat_service: ChatService, interval: Duration) -> JoinHandle<()> {
        let consumer = ChatDispatcher {
            hub_service: service.clone(),
            chat_service,
            cursors: HashMap::new(),
        };
        spawn_connection_loop(&service, interval, consumer).await
    }

    /// Adds cursors for newly seen connections (starting from history) and drops cursors for
    /// connections that are gone.
    fn reconcile(&mut self, tracker: &ConnectionTracker) {
        let live: HashSet<Uuid> = tracker
            .connections()
            .values()
            .map(|(connection_id, _)| *connection_id)
            .collect();
        for connection_id in &live {
            self.cursors
                .entry(*connection_id)
                .or_insert_with(|| INITIAL_CURSOR.to_string());
        }
        self.cursors.retain(|connection_id, _| live.contains(connection_id));
    }
}

impl ConnectionConsumer for ChatDispatcher {
    async fn on_tick(&mut self, tracker: &ConnectionTracker) {
        self.reconcile(tracker);

        // Oldest cursor across all connections: one query serves everyone, each connection then
        // filtered to the slice newer than its own cursor.
        let Some(oldest) = self.cursors.values().min_by_key(|id| parse_stream_id(id)).cloned() else {
            return;
        };

        let batch = match self.chat_service.list_after(&oldest, BATCH_LIMIT).await {
            Ok(batch) => batch,
            Err(err) => {
                log::error!("Failed to read chat stream from cursor {oldest}: {err:#?}");
                return;
            }
        };
        if batch.is_empty() {
            return;
        }

        // Parse each entry's stream id once, not once per connection.
        let batch: Vec<(StreamId, &StoredChatComment)> = batch
            .iter()
            .map(|entry| (parse_stream_id(&entry.stream_id), entry))
            .collect();

        for (connection_id, cursor) in self.cursors.iter_mut() {
            let cursor_id = parse_stream_id(cursor);
            let fresh = batch.iter().filter(|(id, _)| *id > cursor_id);

            let mut last_id = None;
            let comments: Vec<ChatComment> = fresh
                .map(|(_, entry)| {
                    last_id = Some(entry.stream_id.clone());
                    ChatComment {
                        id: entry.stream_id.clone(),
                        user_id: entry.user_id,
                        text: entry.text.clone(),
                    }
                })
                .collect();
            let Some(last_id) = last_id else {
                continue;
            };

            if let Err(err) = self
                .hub_service
                .send_to_connection(*connection_id, HubMessage::Chat(ChatBatch { comments }))
                .await
            {
                log::error!("[{connection_id}] Failed to deliver chat batch: {err:#?}");
                continue;
            }
            *cursor = last_id;
        }
    }
}

/// A Redis stream id `<ms>-<seq>` parsed into its numeric parts for ordering. An unparsable part
/// parses as 0, which is harmless for the cursor `"0"` sentinel and keeps a malformed id from ever
/// ordering greater than a real one.
type StreamId = (u64, u64);

/// Parses a Redis stream id `<ms>-<seq>` into [`StreamId`]. Ordering the tuples matches Redis'
/// numeric id order, which lexical string order would not (`"10-0"` vs `"9-0"`).
fn parse_stream_id(id: &str) -> StreamId {
    let mut parts = id.splitn(2, '-');
    let ms = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let seq = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (ms, seq)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    #[test]
    fn stream_ids_order_numerically_not_lexically() {
        // Lexical order would rank "10-0" before "9-0"; numeric order must not.
        assert!(parse_stream_id("10-0") > parse_stream_id("9-0"));
        assert!(parse_stream_id("100-5") < parse_stream_id("100-10"));
        assert!(parse_stream_id("5-0") == parse_stream_id("5-0"));
        // The initial sentinel is the smallest possible cursor.
        assert!(parse_stream_id(INITIAL_CURSOR) < parse_stream_id("1-0"));
    }
}
