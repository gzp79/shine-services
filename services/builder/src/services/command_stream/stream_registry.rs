use crate::{
    models::{messages::CommandFailure, CommandStreamError},
    repositories::{
        command_inbox::{CommandInboxDb, InboxWake},
        command_log::CommandLogDb,
    },
    services::command_stream::{
        stream_metrics::{elapsed_seconds, kind_label, stream_metrics},
        StreamConfig, StreamEgress, StreamLogService, StreamWorker,
    },
};
use futures::future::join_all;
use shine_core::tokio::Backpressure;
use shine_game::replica::{decode_command, ReplayState, StreamId, StreamKind};
use shine_infra::telemetry::label;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Longest command, as JSON text, that is queued.
pub const MAX_COMMAND_BYTES: usize = 16 * 1024;

/// What a kind of stream is to the registry. It is made by `register`, which is the only place that
/// knows the type of the replayed state; here it is behind closures.
struct KindEntry {
    spawn: Box<dyn Fn(StreamId) -> Arc<StreamWorker> + Send + Sync>,
    is_valid_command: Box<dyn Fn(&[u8]) -> bool + Send + Sync>,
    backpressure: Box<dyn Fn(StreamId) -> Backpressure + Send + Sync>,
}

/// A stream that is open for at least one connection, or served by a worker.
struct StreamState {
    worker: Option<Arc<StreamWorker>>,
    backpressure: Backpressure,
}

#[derive(Default)]
struct State {
    streams: HashMap<StreamId, StreamState>,
    connections: HashMap<Uuid, HashSet<StreamId>>,
}

impl State {
    fn workers(&self) -> impl Iterator<Item = &Arc<StreamWorker>> {
        self.streams.values().filter_map(|stream| stream.worker.as_ref())
    }

    fn unsubscribe_worker(&self, connection_id: Uuid, stream_id: &StreamId) {
        if let Some(worker) = self.streams.get(stream_id).and_then(|stream| stream.worker.as_ref()) {
            worker.unsubscribe(connection_id);
        }
    }
}

struct Inner<I: CommandInboxDb, L: CommandLogDb> {
    inbox: I,
    log: L,
    kinds: HashMap<StreamKind, KindEntry>,
    state: Mutex<State>,
}

/// The streams this instance serves. A connection opens a stream at once, which lets it push, and its
/// subscribe reaches the registry later, ordered with the lifecycle of the connection, to start or join
/// the stream's worker. The write path checks a command for its kind, waits for the backpressure of the
/// stream's inbox, and queues it there. The workers are woken by the inbox and released with their
/// connections. It does not know the types of the streams' states: each kind is registered with what it
/// takes to start its workers and to check its commands.
pub struct StreamRegistry<I: CommandInboxDb, L: CommandLogDb> {
    inner: Arc<Inner<I, L>>,
}

impl<I: CommandInboxDb, L: CommandLogDb> Clone for StreamRegistry<I, L> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

/// Collects the kinds of streams to serve, at startup. The registry made from it serves exactly those.
pub struct StreamRegistryBuilder<I: CommandInboxDb, L: CommandLogDb> {
    inbox: I,
    log: L,
    kinds: HashMap<StreamKind, KindEntry>,
    duplicate: Option<StreamKind>,
}

impl<I: CommandInboxDb + 'static, L: CommandLogDb + 'static> StreamRegistryBuilder<I, L> {
    /// Registers the kind `S::KIND`: its streams are served over `service` and send through `egress`.
    pub fn register<S, E>(mut self, service: StreamLogService<I, L>, egress: E, config: StreamConfig) -> Self
    where
        S: ReplayState,
        E: StreamEgress,
    {
        let inbox = self.inbox.clone();
        let backpressure = {
            let (inbox, config) = (self.inbox.clone(), config.clone());
            Box::new(move |stream_id| inbox_backpressure(&inbox, stream_id, &config))
        };
        let entry = KindEntry {
            spawn: Box::new(move |stream_id| {
                StreamWorker::start::<S, I, L, E>(
                    service.clone(),
                    inbox.clone(),
                    egress.clone(),
                    stream_id,
                    config.clone(),
                )
            }),
            is_valid_command: Box::new(|data| {
                decode_command::<S>(data).is_ok_and(|command| S::is_well_formed(&command))
            }),
            backpressure,
        };
        if self.kinds.insert(S::KIND, entry).is_some() {
            self.duplicate.get_or_insert(S::KIND);
        }
        self
    }

    /// Fails if a kind was registered twice.
    pub fn build(self) -> Result<StreamRegistry<I, L>, CommandStreamError> {
        if let Some(kind) = self.duplicate {
            return Err(CommandStreamError::DuplicateKind(kind));
        }
        Ok(StreamRegistry {
            inner: Arc::new(Inner {
                log: self.log,
                inbox: self.inbox,
                kinds: self.kinds,
                state: Mutex::new(State::default()),
            }),
        })
    }
}

impl<I: CommandInboxDb + 'static, L: CommandLogDb + 'static> StreamRegistry<I, L> {
    pub fn builder(inbox: I, log: L) -> StreamRegistryBuilder<I, L> {
        StreamRegistryBuilder {
            inbox,
            log,
            kinds: HashMap::new(),
            duplicate: None,
        }
    }

    /// Opens the stream for the connection and accepts commands for it right away.
    pub fn open(&self, connection_id: Uuid, stream_id: &StreamId) -> Result<(), CommandStreamError> {
        let entry = self
            .inner
            .kinds
            .get(&stream_id.kind())
            .ok_or(CommandStreamError::StreamNotFound)?;
        let mut state = self.inner.state.lock().unwrap();
        state.streams.entry(stream_id.clone()).or_insert_with(|| StreamState {
            worker: None,
            backpressure: (entry.backpressure)(stream_id.clone()),
        });
        state
            .connections
            .entry(connection_id)
            .or_default()
            .insert(stream_id.clone());
        Ok(())
    }

    /// Closes the stream for the connection and unsubscribes it from the stream's worker.
    pub fn close(&self, connection_id: Uuid, stream_id: &StreamId) {
        let mut state = self.inner.state.lock().unwrap();
        if let Some(streams) = state.connections.get_mut(&connection_id) {
            streams.remove(stream_id);
            if streams.is_empty() {
                state.connections.remove(&connection_id);
            }
        }
        state.unsubscribe_worker(connection_id, stream_id);
    }

    /// The connection is gone: closes all its streams.
    pub fn disconnect(&self, connection_id: Uuid) {
        let mut state = self.inner.state.lock().unwrap();
        for stream_id in state.connections.remove(&connection_id).unwrap_or_default() {
            state.unsubscribe_worker(connection_id, &stream_id);
        }
    }

    /// Whether `data` is a command of the kind that its worker will take.
    pub fn validate(&self, kind: StreamKind, data: &[u8]) -> bool {
        self.inner
            .kinds
            .get(&kind)
            .is_some_and(|entry| (entry.is_valid_command)(data))
    }

    /// Queues a command of the connection in the inbox of the stream. This is the write path, commands
    /// are enqueued into the inbox. If the inbox is full, the push waits for it to drain, and fails as
    /// `Busy` if it does not in time.
    pub async fn push(
        &self,
        connection_id: Uuid,
        stream_id: &StreamId,
        user_id: Uuid,
        command_id: Uuid,
        command: &[u8],
    ) -> Result<(), CommandFailure> {
        let result = self
            .try_push(connection_id, stream_id, user_id, command_id, command)
            .await;
        let outcome = match &result {
            Ok(()) => "queued",
            Err(CommandFailure::NotSubscribed) => "not_subscribed",
            Err(CommandFailure::Invalid) => "invalid",
            Err(CommandFailure::Busy) => "busy",
            Err(CommandFailure::Failed) => "failed",
        };
        stream_metrics()
            .commands_pushed
            .add(1, &[kind_label(stream_id.kind()), label("outcome", outcome)]);
        result
    }

    async fn try_push(
        &self,
        connection_id: Uuid,
        stream_id: &StreamId,
        user_id: Uuid,
        command_id: Uuid,
        command: &[u8],
    ) -> Result<(), CommandFailure> {
        let backpressure = {
            let state = self.inner.state.lock().unwrap();
            let is_open = state
                .connections
                .get(&connection_id)
                .is_some_and(|streams| streams.contains(stream_id));
            is_open
                .then(|| state.streams.get(stream_id).map(|stream| stream.backpressure.clone()))
                .flatten()
                .ok_or(CommandFailure::NotSubscribed)?
        };

        let Some(entry) = self.inner.kinds.get(&stream_id.kind()) else {
            return Err(CommandFailure::Invalid);
        };
        if command.len() > MAX_COMMAND_BYTES || !(entry.is_valid_command)(command) {
            return Err(CommandFailure::Invalid);
        }

        // Only the pushes that had to wait are measured, a free inbox would flood the histogram with zeros.
        let throttled = backpressure.is_blocked();
        let waiting_since = Instant::now();
        let released = backpressure.wait().await;
        if throttled {
            let outcome = if released.is_ok() { "ok" } else { "timeout" };
            stream_metrics().push_throttle_wait.record(
                elapsed_seconds(waiting_since),
                &[kind_label(stream_id.kind()), label("outcome", outcome)],
            );
        }
        if released.is_err() {
            tracing::info!("[{stream_id}] The inbox stayed full");
            return Err(CommandFailure::Busy);
        }

        self.inner
            .inbox
            .push(stream_id, command_id, user_id, command)
            .await
            .map(|_| ())
            .map_err(|err| {
                tracing::warn!("[{stream_id}] Failed to queue a command: {err:#?}");
                CommandFailure::Failed
            })
    }

    /// Subscribes the connection to the stream's worker. If there is no worker, it starts a new one to process
    /// the commands for the stream in the background.
    pub fn subscribe(&self, connection_id: Uuid, stream_id: &StreamId) -> Result<(), CommandStreamError> {
        let entry = self
            .inner
            .kinds
            .get(&stream_id.kind())
            .ok_or(CommandStreamError::StreamNotFound)?;

        let mut state = self.inner.state.lock().unwrap();

        // Find the open stream for this connection, if any.
        let stream = {
            let is_open = state
                .connections
                .get(&connection_id)
                .is_some_and(|streams| streams.contains(stream_id));

            let Some(stream) = state.streams.get_mut(stream_id).filter(|_| is_open) else {
                return Ok(());
            };
            stream
        };

        // Subscribe to the worker of the stream.
        let running = stream.worker.as_ref().filter(|worker| !worker.is_finished());
        let subscribed = running.is_some_and(|worker| worker.subscribe(connection_id));
        if !subscribed {
            // Start a new worker for the stream.
            let worker = (entry.spawn)(stream_id.clone());
            worker.subscribe(connection_id);
            stream.worker = Some(worker);
        }

        Ok(())
    }

    /// Wakes the worker of the stream, if there is one.
    pub fn wake(&self, stream_id: &StreamId) {
        let state = self.inner.state.lock().unwrap();
        if let Some(worker) = state.streams.get(stream_id).and_then(|stream| stream.worker.as_ref()) {
            worker.wake();
        }
    }

    pub fn wake_all(&self) {
        for worker in self.inner.state.lock().unwrap().workers() {
            worker.wake();
        }
    }

    /// The workers that are running.
    pub fn worker_count(&self) -> usize {
        self.inner
            .state
            .lock()
            .unwrap()
            .workers()
            .filter(|worker| !worker.is_finished())
            .count()
    }

    /// Forgets the streams that no connection has open and whose worker, if any, has ended. A new
    /// subscribe replaces an ended worker anyway; this only releases the streams nobody comes back to.
    pub fn purge(&self) {
        let mut state = self.inner.state.lock().unwrap();
        let State { streams, connections } = &mut *state;
        let open: HashSet<&StreamId> = connections.values().flatten().collect();
        streams.retain(|stream_id, stream| {
            open.contains(stream_id) || stream.worker.as_ref().is_some_and(|worker| !worker.is_finished())
        });
    }

    /// All the held workers.
    pub fn workers_count(&self) -> usize {
        self.inner.state.lock().unwrap().workers().count()
    }

    /// Starts to wake the workers on the wakes of the inbox. A listener that reconnected may have missed
    /// wakes, so it wakes every worker.
    pub async fn listen(&self) -> Result<(), CommandStreamError> {
        let registry = self.clone();
        self.inner
            .inbox
            .listen(move |wake| match wake {
                InboxWake::Stream { stream_id, .. } => registry.wake(&stream_id),
                InboxWake::All => registry.wake_all(),
            })
            .await
    }

    pub async fn unlisten(&self) -> Result<(), CommandStreamError> {
        self.inner.inbox.unlisten().await
    }

    /// Asks every worker to persist what is waiting and end.
    pub async fn shutdown(&self, timeout: Duration) {
        let workers: Vec<Arc<StreamWorker>> = self.inner.state.lock().unwrap().workers().cloned().collect();
        for worker in &workers {
            worker.stop();
        }
        let ended = join_all(workers.iter().map(|worker| worker.wait_finished()));
        if tokio::time::timeout(timeout, ended).await.is_err() {
            tracing::warn!("Some stream workers did not end within {timeout:?}");
        }
        self.purge();
    }
}

/// Polls the length of the stream's inbox; it lives as long as a handle of it is held.
fn inbox_backpressure<I: CommandInboxDb + 'static>(
    inbox: &I,
    stream_id: StreamId,
    config: &StreamConfig,
) -> Backpressure {
    let inbox = inbox.clone();
    Backpressure::new(
        config.inbox_backpressure_low,
        config.inbox_backpressure_high,
        config.inbox_backpressure_poll_period,
        Some(config.inbox_backpressure_timeout),
        move || {
            let (inbox, stream_id) = (inbox.clone(), stream_id.clone());
            async move {
                // An unreadable length does not block the pushes: they fail on their own if the inbox is down.
                match inbox.len(&stream_id).await {
                    Ok(len) => {
                        stream_metrics()
                            .inbox_depth
                            .record(len as u64, &[kind_label(stream_id.kind())]);
                        len
                    }
                    Err(err) => {
                        tracing::warn!("[{stream_id}] Failed to read the inbox length: {err:#?}");
                        0
                    }
                }
            }
        },
    )
}

#[cfg(test)]
#[path = "stream_registry_test.rs"]
mod stream_registry_test;
