use crate::{
    models::{messages::CommandFailure, CommandStreamError},
    repositories::{
        command_inbox::{CommandInboxDb, InboxWake},
        command_log::{CommandLogDb, StreamId},
    },
    services::command_stream::{spawn_worker, CommandStreamService, StreamEgress, WorkerConfig, WorkerHandle},
};
use futures::future::join_all;
use shine_game::replica::{decode_command, ReplayState, StreamKind};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

/// Longest command, as JSON text, that is queued.
pub const MAX_COMMAND_BYTES: usize = 16 * 1024;

/// What a kind of stream is to the registry. It is made by `register`, which is the only place that
/// knows the type of the replayed state; here it is behind closures.
struct KindEntry {
    spawn: Box<dyn Fn(StreamId) -> WorkerHandle + Send + Sync>,
    is_valid_command: Box<dyn Fn(&str) -> bool + Send + Sync>,
}

#[derive(Default)]
struct State {
    workers: HashMap<StreamId, WorkerHandle>,
    /// The streams each connection is subscribed to, to release them when it leaves.
    connections: HashMap<Uuid, HashSet<StreamId>>,
}

struct Inner<I: CommandInboxDb, L: CommandLogDb> {
    inbox: I,
    log: L,
    kinds: HashMap<StreamKind, KindEntry>,
    state: Mutex<State>,
}

/// The streams this instance serves. Its write path checks a command for its kind and queues it in the
/// inbox of its stream. Its workers are found or started when a connection subscribes, woken by the
/// inbox, and released with the connection. It does not know the types of the streams' states: each
/// kind is registered with what it takes to start its workers and to check its commands.
// todo: swap generic args
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
    /// Registers the kind `S::KIND`: its workers are started over `service` and send through `egress`.
    pub fn register<S, E>(mut self, service: CommandStreamService<I, L>, egress: E, config: WorkerConfig) -> Self
    where
        S: ReplayState,
        E: StreamEgress,
    {
        let entry = KindEntry {
            spawn: Box::new(move |stream_id| {
                spawn_worker::<S, I, L, E>(service.clone(), egress.clone(), stream_id, config.clone())
            }),
            is_valid_command: Box::new(|data| {
                decode_command::<S>(data).is_ok_and(|command| S::is_well_formed(&command))
            }),
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

    /// Validates that the stream's known kind is registered before it is opened.
    pub async fn open(&self, stream_id: &StreamId) -> Result<StreamKind, CommandStreamError> {
        let kind = stream_id.kind();
        if !self.inner.kinds.contains_key(&kind) {
            return Err(CommandStreamError::StreamNotFound);
        }
        Ok(kind)
    }

    /// Whether `data` is a command of the kind that its worker will take.
    pub fn validate(&self, kind: StreamKind, data: &str) -> bool {
        self.inner
            .kinds
            .get(&kind)
            .is_some_and(|entry| (entry.is_valid_command)(data))
    }

    /// Queues a command of the stream, of the given kind, in its inbox, if it is valid for the kind. This
    /// is the write path: the workers do the rest.
    pub async fn push(
        &self,
        stream_id: &StreamId,
        user_id: Uuid,
        command_id: Uuid,
        command: &str,
    ) -> Result<(), CommandFailure> {
        if command.len() > MAX_COMMAND_BYTES || !self.validate(stream_id.kind(), command) {
            return Err(CommandFailure::Invalid);
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

    /// Subscribes the connection to the stream, starting its worker if there is none.
    pub fn subscribe(&self, connection_id: Uuid, stream_id: &StreamId) -> Result<(), CommandStreamError> {
        let entry = self
            .inner
            .kinds
            .get(&stream_id.kind())
            .ok_or(CommandStreamError::StreamNotFound)?;
        let mut state = self.inner.state.lock().unwrap();

        let running = state
            .workers
            .get(stream_id)
            .filter(|worker| !worker.is_finished())
            .cloned();
        let subscribed = running.is_some_and(|worker| worker.subscribe(connection_id));
        if !subscribed {
            // None, or one that ended just now: start a new one, which takes the subscriber at once.
            let worker = (entry.spawn)(stream_id.clone());
            worker.subscribe(connection_id);
            state.workers.insert(stream_id.clone(), worker);
        }
        state
            .connections
            .entry(connection_id)
            .or_default()
            .insert(stream_id.clone());
        Ok(())
    }

    pub fn unsubscribe(&self, connection_id: Uuid, stream_id: &StreamId) {
        let mut state = self.inner.state.lock().unwrap();
        if let Some(streams) = state.connections.get_mut(&connection_id) {
            streams.remove(stream_id);
            if streams.is_empty() {
                state.connections.remove(&connection_id);
            }
        }
        if let Some(worker) = state.workers.get(stream_id) {
            worker.unsubscribe(connection_id);
        }
    }

    /// The connection is gone: unsubscribes it from all its streams.
    pub fn disconnect(&self, connection_id: Uuid) {
        let mut state = self.inner.state.lock().unwrap();
        for stream_id in state.connections.remove(&connection_id).unwrap_or_default() {
            if let Some(worker) = state.workers.get(&stream_id) {
                worker.unsubscribe(connection_id);
            }
        }
    }

    /// Wakes the worker of the stream, if there is one.
    pub fn wake(&self, stream_id: &StreamId) {
        if let Some(worker) = self.inner.state.lock().unwrap().workers.get(stream_id) {
            worker.wake();
        }
    }

    pub fn wake_all(&self) {
        for worker in self.inner.state.lock().unwrap().workers.values() {
            worker.wake();
        }
    }

    /// The workers that are running.
    pub fn worker_count(&self) -> usize {
        self.inner
            .state
            .lock()
            .unwrap()
            .workers
            .values()
            .filter(|worker| !worker.is_finished())
            .count()
    }

    /// Forgets the workers that have ended. A new subscribe replaces an ended worker anyway; this only
    /// releases the entries of streams nobody comes back to.
    pub fn purge(&self) {
        self.inner
            .state
            .lock()
            .unwrap()
            .workers
            .retain(|_, worker| !worker.is_finished());
    }

    /// The workers held, running or ended.
    #[cfg(test)]
    fn held_workers(&self) -> usize {
        self.inner.state.lock().unwrap().workers.len()
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

    /// Asks every worker to persist what is waiting and end, and waits for them for up to `timeout`.
    pub async fn shutdown(&self, timeout: Duration) {
        let workers: Vec<WorkerHandle> = self.inner.state.lock().unwrap().workers.values().cloned().collect();
        for worker in &workers {
            worker.stop();
        }
        let ended = join_all(workers.iter().map(|worker| worker.finished()));
        if tokio::time::timeout(timeout, ended).await.is_err() {
            tracing::warn!("Some stream workers did not end within {timeout:?}");
        }
        self.purge();
    }
}

#[cfg(test)]
#[path = "stream_registry_test.rs"]
mod stream_registry_test;
