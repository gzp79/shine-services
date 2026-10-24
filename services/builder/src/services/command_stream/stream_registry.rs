use crate::{
    models::{CommandInboxError, CommandLogError},
    repositories::{
        command_inbox::{CommandInboxDb, InboxWake},
        command_log::{CommandLogDb, StreamId},
    },
    services::command_stream::{spawn_worker, CommandStreamService, StreamEgress, WorkerConfig, WorkerHandle},
};
use futures::future::join_all;
use shine_game::replica::{decode_command, ReplayState};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum RegistryError {
    #[error("Unknown stream kind {0:?}")]
    UnknownKind(String),
    #[error("The stream kind {0:?} is registered twice")]
    DuplicateKind(String),
    #[error("The stream is of kind {stored:?}, not {requested:?}")]
    KindMismatch { stored: String, requested: String },
    #[error(transparent)]
    Log(#[from] CommandLogError),
}

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

struct Inner<L: CommandLogDb> {
    log: L,
    /// Fixed at startup, so it needs no lock.
    kinds: HashMap<String, KindEntry>,
    state: Mutex<State>,
}

/// The workers of the streams this instance serves: finds or starts the worker of a stream when a
/// connection subscribes, routes the wakes of the inbox to the workers, and releases a connection's
/// streams when it leaves. It does not know the types of the streams' states: each kind is registered
/// with what it takes to start its workers.
pub struct StreamRegistry<L: CommandLogDb> {
    inner: Arc<Inner<L>>,
}

impl<L: CommandLogDb> Clone for StreamRegistry<L> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

/// Collects the kinds of streams to serve, at startup. The registry made from it serves exactly those.
pub struct StreamRegistryBuilder<L: CommandLogDb> {
    log: L,
    kinds: HashMap<String, KindEntry>,
    duplicate: Option<String>,
}

impl<L: CommandLogDb + 'static> StreamRegistryBuilder<L> {
    /// Registers the kind `S::KIND`: its workers are started over `service` and send through `egress`.
    pub fn register<S, I, E>(mut self, service: CommandStreamService<L, I>, egress: E, config: WorkerConfig) -> Self
    where
        S: ReplayState,
        I: CommandInboxDb + 'static,
        E: StreamEgress,
    {
        let entry = KindEntry {
            spawn: Box::new(move |stream_id| {
                spawn_worker::<S, L, I, E>(service.clone(), egress.clone(), stream_id, config.clone())
            }),
            is_valid_command: Box::new(|data| {
                decode_command::<S>(data).is_ok_and(|command| S::is_well_formed(&command))
            }),
        };
        if self.kinds.insert(S::KIND.to_string(), entry).is_some() {
            self.duplicate.get_or_insert_with(|| S::KIND.to_string());
        }
        self
    }

    /// Fails if a kind was registered twice.
    pub fn build(self) -> Result<StreamRegistry<L>, RegistryError> {
        if let Some(kind) = self.duplicate {
            return Err(RegistryError::DuplicateKind(kind));
        }
        Ok(StreamRegistry {
            inner: Arc::new(Inner {
                log: self.log,
                kinds: self.kinds,
                state: Mutex::new(State::default()),
            }),
        })
    }
}

impl<L: CommandLogDb + 'static> StreamRegistry<L> {
    pub fn builder(log: L) -> StreamRegistryBuilder<L> {
        StreamRegistryBuilder {
            log,
            kinds: HashMap::new(),
            duplicate: None,
        }
    }

    /// Checks that a connection may subscribe to the stream as `requested`, and returns its kind. The
    /// kind of an existing stream is the one it was created with; a new stream takes the requested one.
    pub async fn open(&self, stream_id: &StreamId, requested: &str) -> Result<String, RegistryError> {
        let kind = match self.inner.log.head(stream_id).await? {
            Some(head) if head.kind != requested => {
                return Err(RegistryError::KindMismatch {
                    stored: head.kind,
                    requested: requested.to_string(),
                })
            }
            Some(head) => head.kind,
            None => requested.to_string(),
        };
        if !self.inner.kinds.contains_key(&kind) {
            return Err(RegistryError::UnknownKind(kind));
        }
        Ok(kind)
    }

    /// Whether `data` is a command of the kind that its worker will take.
    pub fn validate(&self, kind: &str, data: &str) -> bool {
        self.inner
            .kinds
            .get(kind)
            .is_some_and(|entry| (entry.is_valid_command)(data))
    }

    /// Subscribes the connection to the stream, starting its worker if there is none. The kind comes
    /// from `open`.
    pub fn subscribe(&self, connection_id: Uuid, stream_id: &StreamId, kind: &str) -> Result<(), RegistryError> {
        let entry = self
            .inner
            .kinds
            .get(kind)
            .ok_or_else(|| RegistryError::UnknownKind(kind.to_string()))?;
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
            self.watch(stream_id, &worker);
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

    pub fn worker_count(&self) -> usize {
        self.inner.state.lock().unwrap().workers.len()
    }

    /// Starts to wake the workers on the wakes of the inbox. A listener that reconnected may have missed
    /// wakes, so it wakes every worker.
    pub async fn listen<I: CommandInboxDb>(&self, inbox: &I) -> Result<(), CommandInboxError> {
        let registry = self.clone();
        inbox
            .listen(move |wake| match wake {
                InboxWake::Stream { stream_id, .. } => registry.wake(&stream_id),
                InboxWake::All => registry.wake_all(),
            })
            .await
    }

    pub async fn unlisten<I: CommandInboxDb>(&self, inbox: &I) -> Result<(), CommandInboxError> {
        inbox.unlisten().await
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
    }

    /// Forgets the worker when it ends, unless a new one has taken its place.
    fn watch(&self, stream_id: &StreamId, worker: &WorkerHandle) {
        let registry = self.clone();
        let (stream_id, worker) = (stream_id.clone(), worker.clone());
        tokio::spawn(async move {
            worker.finished().await;
            let mut state = registry.inner.state.lock().unwrap();
            if state
                .workers
                .get(&stream_id)
                .is_some_and(|current| current.same_worker(&worker))
            {
                state.workers.remove(&stream_id);
            }
        });
    }
}

#[cfg(test)]
#[path = "stream_registry_test.rs"]
mod stream_registry_test;
