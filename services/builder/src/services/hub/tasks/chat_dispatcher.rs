use super::connection_tracker::{spawn_connection_loop_with_wake, ConnectionConsumer, ConnectionTracker};
use crate::{
    models::messages::{ChatBatch, ChatComment, HubMessage},
    repositories::chat_comments::ChatPage,
    services::{hub_metrics, ChatService, HubService},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{sync::Notify, task::JoinHandle};
use tracing::{trace_span, Instrument};
use uuid::Uuid;

/// Max entries read from the stream in a single page.
const BATCH_LIMIT: usize = 128;

/// Pages read per group of connections in a single tick. A group further behind than this catches
/// up over the following ticks, so one tick stays bounded.
const MAX_PAGES_PER_TICK: usize = 8;

/// Ticks between two checks that the stream did not restart behind the connections' cursors.
const REGRESSION_CHECK_TICKS: u32 = 30;

/// The initial cursor for a freshly seen connection: `list_after` treats it exclusively, so `"0"`
/// yields all currently retained history.
const INITIAL_CURSOR: &str = "0";

/// Consumer that pushes per-connection chat batches to connected users.
pub struct ChatDispatcher {
    hub_service: HubService,
    chat_service: ChatService,
    cursors: HashMap<Uuid, String>,
    regression_check_every: Duration,
    last_regression_check: Instant,
}

impl ChatDispatcher {
    /// Starts the chat dispatcher on its own connection loop, delivering on each periodic tick and,
    /// ahead of it, whenever a Redis pub/sub append notification wakes the loop.
    pub async fn start(service: HubService, chat_service: ChatService, interval: Duration) -> JoinHandle<()> {
        let wake = Arc::new(Notify::new());

        // Register the append listener before the loop starts so no notification is missed. A failure
        // here is non-fatal: the periodic tick still delivers, only without the push latency.
        let notify = wake.clone();
        if let Err(err) = chat_service.listen_to_comments(move || notify.notify_one()).await {
            tracing::error!("Failed to listen for chat appends; falling back to periodic delivery only: {err:#?}");
        }

        let consumer = ChatDispatcher {
            hub_service: service.clone(),
            chat_service,
            cursors: HashMap::new(),
            regression_check_every: interval.saturating_mul(REGRESSION_CHECK_TICKS),
            last_regression_check: Instant::now(),
        };
        spawn_connection_loop_with_wake(&service, interval, wake, consumer).await
    }

    /// Adds cursors for newly seen connections and drops cursors for connections that are gone.
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

    /// Reads and delivers the comments after `cursor` to `members`, which all sit at that cursor,
    /// until they are caught up or the tick's page budget is spent. The members share each page's
    /// read and its batch.
    async fn deliver_group(&mut self, mut cursor: String, mut members: Vec<Uuid>) {
        for _ in 0..MAX_PAGES_PER_TICK {
            let ChatPage { comments, last_id, more } = match self.chat_service.list_after(&cursor, BATCH_LIMIT).await {
                Ok(page) => page,
                Err(err) => {
                    tracing::error!("Failed to read chat stream from cursor {cursor}: {err:#?}");
                    return;
                }
            };
            let Some(last_id) = last_id else {
                return;
            };

            // Unreadable entries leave no comments, yet the cursors still move past them.
            let batch = (!comments.is_empty()).then(|| {
                ChatBatch::new(
                    comments
                        .into_iter()
                        .map(|entry| ChatComment {
                            id: entry.stream_id,
                            user_id: entry.user_id,
                            text: entry.text,
                        })
                        .collect(),
                )
            });

            let mut caught_up = Vec::with_capacity(members.len());
            for connection_id in members {
                let queued = match &batch {
                    Some(batch) => {
                        match self
                            .hub_service
                            .send_to_connection(connection_id, HubMessage::Chat(batch.clone()))
                            .await
                        {
                            Ok(queued) => queued,
                            // The connection keeps its cursor and is tried again next tick.
                            Err(err) => {
                                tracing::error!("[{connection_id}] Failed to deliver chat batch: {err:#?}");
                                continue;
                            }
                        }
                    }
                    None => false,
                };
                if let (true, Some(batch)) = (queued, &batch) {
                    hub_metrics().chat_delivered.add(batch.comments.len() as u64, &[]);
                }
                // Also advanced for an unknown or dead connection, which would otherwise hold its
                // group back until reconcile drops it.
                if let Some(connection_cursor) = self.cursors.get_mut(&connection_id) {
                    *connection_cursor = last_id.clone();
                }
                caught_up.push(connection_id);
            }

            members = caught_up;
            cursor = last_id;
            if !more || members.is_empty() {
                return;
            }
        }
    }

    /// Resets the cursors when the stream restarted behind them: after a flush, a restore or a
    /// failover, new entries get ids below every cursor and would never be delivered.
    async fn reset_regressed_cursors(&mut self) {
        let newest = match self.chat_service.last_id().await {
            Ok(newest) => newest.map(|id| parse_stream_id(&id)).unwrap_or_default(),
            Err(err) => {
                tracing::error!("Failed to read the newest chat entry: {err:#?}");
                return;
            }
        };

        let mut reset = 0;
        for cursor in self.cursors.values_mut() {
            if parse_stream_id(cursor) > newest {
                *cursor = INITIAL_CURSOR.to_string();
                reset += 1;
            }
        }
        if reset > 0 {
            tracing::warn!("The chat stream restarted behind {reset} connections; replaying it to them");
        }
    }
}

impl ConnectionConsumer for ChatDispatcher {
    #[tracing::instrument(level = "trace", name = "hub chat dispatch", skip_all)]
    async fn on_tick(&mut self, tracker: &ConnectionTracker) {
        self.reconcile(tracker);
        if self.cursors.is_empty() {
            return;
        }

        // Connections at the same cursor share one read: the caught-up ones, and newcomers replaying
        // the history from the start, who must not hold the others back.
        let mut groups: HashMap<String, Vec<Uuid>> = HashMap::new();
        for (connection_id, cursor) in &self.cursors {
            groups.entry(cursor.clone()).or_default().push(*connection_id);
        }

        let span = trace_span!("chat deliver", hub.groups = groups.len());
        async {
            for (cursor, members) in groups {
                self.deliver_group(cursor, members).await;
            }
        }
        .instrument(span)
        .await;

        if self.last_regression_check.elapsed() >= self.regression_check_every {
            self.last_regression_check = Instant::now();
            self.reset_regressed_cursors().await;
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
#[path = "chat_dispatcher_test.rs"]
mod chat_dispatcher_test;
