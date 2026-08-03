use crate::sync::ExponentialBackoff;
use futures::StreamExt;
use redis::{
    aio::{PubSubSink, PubSubStream},
    Client, RedisError,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use thiserror::Error as ThisError;
use tokio::{
    sync::{Notify, RwLock},
    task::JoinHandle,
    time::{Duration, Instant},
};

type BoxedHandler = Box<dyn Fn(Option<&str>) + Send + Sync + 'static>;

/// A handler blocking the dispatch task longer than this is logged as a warning.
const SLOW_HANDLER: Duration = Duration::from_millis(100);

#[derive(Debug, ThisError)]
pub enum RedisListenerError {
    #[error(transparent)]
    Redis(#[from] RedisError),
    #[error("The listener has been closed")]
    Closed,
    #[error("A handler is already registered for channel {0:?}")]
    AlreadyListening(String),
}

/// The dedicated pub/sub connection — a `PubSubSink` (subscribe half) plus the streaming task
/// draining its `PubSubStream` — and the channel→handler map it serves.
struct ListenState {
    sink: Option<PubSubSink>,
    stream_task: Option<JoinHandle<()>>,
    handlers: HashMap<String, BoxedHandler>,
}

impl ListenState {
    fn new() -> Self {
        Self {
            sink: None,
            stream_task: None,
            handlers: HashMap::new(),
        }
    }

    fn set_sink(&mut self, sink: PubSubSink, task: JoinHandle<()>) {
        self.sink = Some(sink);
        self.stream_task = Some(task);
    }

    /// Drops the sink so the next connect re-establishes the connection; the streaming task ends on
    /// its own when its stream closes.
    fn disconnect(&mut self) {
        log::info!("RedisListener disconnecting from Redis...");
        self.sink = None;
    }

    /// Tears down the connection and aborts the streaming task (close / drop).
    fn shutdown(&mut self) {
        self.sink = None;
        if let Some(task) = self.stream_task.take() {
            task.abort();
        }
    }

    fn handle(&self, channel: &str, payload: Option<&str>) {
        if let Some(handler) = self.handlers.get(channel) {
            Self::invoke(channel, handler, payload);
        }
    }

    fn handle_reconnect(&self) {
        for (channel, handler) in &self.handlers {
            Self::invoke(channel, handler, None);
        }
    }

    fn invoke(channel: &str, handler: &BoxedHandler, payload: Option<&str>) {
        let started = Instant::now();
        handler(payload);
        let elapsed = started.elapsed();
        if elapsed >= SLOW_HANDLER {
            log::warn!("RedisListener handler for channel {channel:?} took {elapsed:?}, blocking dispatch");
        }
    }
}

struct Inner {
    client: Client,
    notify_keep_alive: Arc<(Notify, AtomicBool)>,
    /// Guards the connection + handler map. Unlike PGListener it may be held across subscribe:
    /// redis-rs drives its own pub/sub connection, so the write lock also serves as the "one
    /// subscription change at a time" serializer. See the design doc, "Concurrency model".
    state: Arc<RwLock<ListenState>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Last handle gone: stop the keep-alive task and wake it so it tears the connection down.
        self.notify_keep_alive.1.store(false, Ordering::Relaxed);
        self.notify_keep_alive.0.notify_one();
    }
}

/// Subscribes to Redis pub/sub channels through one shared, reconnecting connection — the Redis
/// counterpart of `PGListener`. See the design doc `docs/shared/db-pooling-and-listening.html`.
#[derive(Clone)]
pub struct RedisListener {
    inner: Arc<Inner>,
}

impl RedisListener {
    fn start_keep_alive_task(
        client: Client,
        state: Arc<RwLock<ListenState>>,
        notify_keep_alive: Arc<(Notify, AtomicBool)>,
        max_backoff: Duration,
    ) {
        tokio::spawn(async move {
            const RETRY_MIN: Duration = Duration::from_millis(500);
            // A connection that stayed up at least this long is considered healthy and resets the backoff.
            const STABLE: Duration = Duration::from_secs(10);
            let mut backoff = ExponentialBackoff::new(RETRY_MIN, max_backoff);

            notify_keep_alive.0.notified().await;
            while notify_keep_alive.1.load(Ordering::Relaxed) {
                log::info!("RedisListener reconnection triggered...");

                // Connect under the write lock so a concurrent listen() cannot open a second
                // connection; the lock is released before parking below.
                let established = {
                    let mut guard = state.write().await;
                    if !notify_keep_alive.1.load(Ordering::Relaxed) {
                        break;
                    }
                    Self::connect_and_subscribe(&mut guard, &state, &client, &notify_keep_alive).await
                };
                match established {
                    Ok(true) => {
                        log::info!("RedisListener reconnected to Redis.");
                        let connected_at = Instant::now();
                        notify_keep_alive.0.notified().await;
                        if connected_at.elapsed() >= STABLE {
                            backoff.reset();
                        }
                        if notify_keep_alive.1.load(Ordering::Relaxed) {
                            backoff.delay().await;
                        }
                    }
                    Ok(false) => {
                        // Already connected (listen won the race); park for the next trigger.
                        notify_keep_alive.0.notified().await;
                    }
                    Err(err) => {
                        log::error!("RedisListener reconnection error: {err:#?}");
                        backoff.delay().await;
                    }
                }
            }
            state.write().await.shutdown();
            log::info!("RedisListener keep alive is closed");
        });
    }

    /// Opens the shared connection if absent, spawns its streaming task, re-subscribes every
    /// registered channel, and nudges their handlers. Returns whether a new connection was opened.
    /// Runs under the caller's `state` write lock; `state` is passed only to hand a clone to the
    /// streaming task (which locks later, after this guard is released).
    async fn connect_and_subscribe(
        state_guard: &mut ListenState,
        state: &Arc<RwLock<ListenState>>,
        client: &Client,
        notify_keep_alive: &Arc<(Notify, AtomicBool)>,
    ) -> Result<bool, RedisError> {
        if state_guard.sink.is_some() {
            return Ok(false);
        }

        log::trace!("RedisListener connecting to Redis...");
        let pubsub = client.get_async_pubsub().await?;
        let (mut sink, stream) = pubsub.split();
        for channel in state_guard.handlers.keys() {
            log::info!("RedisListener subscribing to channel {channel:?}...");
            sink.subscribe(channel).await?;
        }
        let task = Self::start_streaming_task(state.clone(), stream, notify_keep_alive.clone());
        state_guard.set_sink(sink, task);
        state_guard.handle_reconnect();

        Ok(true)
    }

    fn start_streaming_task(
        state: Arc<RwLock<ListenState>>,
        mut stream: PubSubStream,
        notify_keep_alive: Arc<(Notify, AtomicBool)>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                let channel = msg.get_channel_name().to_string();
                match msg.get_payload::<String>() {
                    Ok(payload) => state.read().await.handle(&channel, Some(&payload)),
                    Err(err) => log::warn!("RedisListener dropping non-UTF-8 payload on channel {channel:?}: {err:#?}"),
                }
            }

            state.write().await.disconnect();

            if notify_keep_alive.1.load(Ordering::Relaxed) {
                log::info!("RedisListener triggering a reconnection for connection lost...");
                notify_keep_alive.0.notify_one();
            }
        })
    }

    pub fn new(client: Client, max_backoff: Duration) -> Self {
        let notify_keep_alive = Arc::new((Notify::new(), AtomicBool::new(true)));
        let state = Arc::new(RwLock::new(ListenState::new()));
        Self::start_keep_alive_task(client.clone(), state.clone(), notify_keep_alive.clone(), max_backoff);

        Self {
            inner: Arc::new(Inner {
                client,
                notify_keep_alive,
                state,
            }),
        }
    }

    /// Stops the keep-alive task and tears down the shared connection.
    pub async fn close(&self) {
        // Set the closed flag and tear down under the write lock: a concurrent listen() checks the
        // flag under the same lock, so it can never leave an unmanaged connection behind.
        {
            let mut state = self.inner.state.write().await;
            self.inner.notify_keep_alive.1.store(false, Ordering::Relaxed);
            state.shutdown();
        }
        self.inner.notify_keep_alive.0.notify_one();
    }

    /// Registers `handler` for `channel`, opening the shared connection on first use.
    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), RedisListenerError>
    where
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        // One write-lock span across the closed check, connect, and subscribe. Holding it that long
        // is safe here (redis-rs drives its own connection) and gives, for free, both the "one
        // subscription change at a time" serialization and the close-vs-listen safety: close() sets
        // the closed flag under the same lock.
        let mut state = self.inner.state.write().await;

        if !self.inner.notify_keep_alive.1.load(Ordering::Relaxed) {
            return Err(RedisListenerError::Closed);
        }
        if state.handlers.contains_key(channel) {
            return Err(RedisListenerError::AlreadyListening(channel.to_string()));
        }

        Self::connect_and_subscribe(
            &mut state,
            &self.inner.state,
            &self.inner.client,
            &self.inner.notify_keep_alive,
        )
        .await?;

        if let Some(sink) = state.sink.as_mut() {
            log::info!("RedisListener subscribing to channel {channel:?}...");
            sink.subscribe(channel).await?;
        }
        state.handlers.insert(channel.to_string(), Box::new(handler));
        Ok(())
    }

    /// Removes `channel`'s handler and unsubscribes on the shared connection, if connected.
    pub async fn unlisten(&self, channel: &str) -> Result<(), RedisListenerError> {
        let mut state = self.inner.state.write().await;
        if state.handlers.remove(channel).is_none() {
            return Ok(());
        }
        if let Some(sink) = state.sink.as_mut() {
            log::info!("RedisListener unsubscribing from channel {channel:?}...");
            sink.unsubscribe(channel).await?;
        }
        Ok(())
    }

    /// Removes all handlers and unsubscribes from every channel on the shared connection.
    pub async fn unlisten_all(&self) -> Result<(), RedisListenerError> {
        let mut state = self.inner.state.write().await;
        let channels = state.handlers.drain().map(|(channel, _)| channel).collect::<Vec<_>>();
        if let Some(sink) = state.sink.as_mut() {
            for channel in channels {
                log::info!("RedisListener unsubscribing from channel {channel:?}...");
                sink.unsubscribe(&channel).await?;
            }
        }
        Ok(())
    }
}
