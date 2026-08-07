use super::connection_tracker::{run_connection_loop, ConnectionConsumer, ConnectionTracker};
use crate::{
    models::messages::{ChatBatch, ChatComment, HubMessage, TopicKey},
    repositories::chat_comments::StoredChatComment,
    services::{ChatService, HubService},
};
use std::{collections::HashMap, time::Duration};
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
    /// and delivering per-connection, id-targeted chat batches. Subscribes synchronously (awaited)
    /// before spawning so the subscription is in place before the command loop starts dispatching.
    pub async fn start(service: HubService, chat_service: ChatService, interval: Duration) -> JoinHandle<()> {
        let subscription = service.subscribe(vec![TopicKey::Hub]).await;
        tokio::spawn(async move {
            let dispatcher = ChatDispatcher {
                hub_service: service,
                chat_service,
                cursors: HashMap::new(),
            };
            run_connection_loop(subscription, interval, dispatcher).await;
        })
    }

    /// Adds cursors for newly seen connections (starting from history) and drops cursors for
    /// connections that are gone.
    fn reconcile(&mut self, tracker: &ConnectionTracker) {
        let live = tracker.connections();
        for connection_id in live.values().map(|(connection_id, _)| *connection_id) {
            self.cursors
                .entry(connection_id)
                .or_insert_with(|| INITIAL_CURSOR.to_string());
        }
        self.cursors
            .retain(|connection_id, _| live.values().any(|(live_id, _)| live_id == connection_id));
    }
}

impl ConnectionConsumer for ChatDispatcher {
    async fn on_tick(&mut self, tracker: &ConnectionTracker) {
        self.reconcile(tracker);

        // Oldest cursor across all connections: one query serves everyone, each connection then
        // filtered to the slice newer than its own cursor.
        let Some(oldest) = self.cursors.values().min_by(|a, b| cmp_stream_id(a, b)).cloned() else {
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

        for (connection_id, cursor) in self.cursors.iter_mut() {
            let fresh: Vec<&StoredChatComment> = batch
                .iter()
                .filter(|entry| cmp_stream_id(&entry.stream_id, cursor).is_gt())
                .collect();
            let Some(last_id) = fresh.last().map(|entry| entry.stream_id.clone()) else {
                continue;
            };

            let comments = fresh
                .into_iter()
                .map(|entry| ChatComment {
                    id: entry.stream_id.clone(),
                    user_id: entry.user_id,
                    text: entry.text.clone(),
                })
                .collect();

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

/// Orders Redis stream ids by their `<ms>-<seq>` parts numerically. An unparsable part sorts as 0,
/// which is harmless for the cursor `"0"` sentinel and keeps a malformed id from ever comparing
/// greater than a real one.
fn cmp_stream_id(a: &str, b: &str) -> std::cmp::Ordering {
    fn parse(id: &str) -> (u64, u64) {
        let mut parts = id.splitn(2, '-');
        let ms = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let seq = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        (ms, seq)
    }
    parse(a).cmp(&parse(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    #[test]
    fn stream_ids_order_numerically_not_lexically() {
        // Lexical order would rank "10-0" before "9-0"; numeric order must not.
        assert!(cmp_stream_id("10-0", "9-0").is_gt());
        assert!(cmp_stream_id("100-5", "100-10").is_lt());
        assert!(cmp_stream_id("5-0", "5-0").is_eq());
        // The initial sentinel is the smallest possible cursor.
        assert!(cmp_stream_id(INITIAL_CURSOR, "1-0").is_lt());
    }
}
