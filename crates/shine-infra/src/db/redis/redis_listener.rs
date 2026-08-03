use crate::sync::{ExponentialBackoff, KeepAlive};
use futures::StreamExt;
use redis::{
    aio::{PubSubSink, PubSubStream},
    Client, ErrorKind, RedisError,
};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error as ThisError;
use tokio::{
    sync::{RwLock, Semaphore},
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
    connect_timeout: Duration,
    keep_alive: Arc<KeepAlive>,
    /// Serializes every subscription/lifecycle change; see the design doc, "Concurrency model".
    op_lock: Arc<Semaphore>,
    /// Guards the sink handle + handler map. Never held across a network command.
    state: Arc<RwLock<ListenState>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Last handle gone: stop the keep-alive task so it tears the connection down.
        self.keep_alive.stop();
    }
}

/// Subscribes to Redis pub/sub channels through one shared, reconnecting connection — the Redis
/// counterpart of `PGListener`. See the design doc `docs/shared/db-pooling-and-listening.html`.
#[derive(Clone)]
pub struct RedisListener {
    inner: Arc<Inner>,
}

impl RedisListener {
    /// Bound for a pub/sub connect/subscribe/unsubscribe when the connection string sets no
    /// `timeout`. redis-rs applies its connect/response timeouts only on the multiplexed path, not
    /// pub/sub, so without this a `listen`/`unlisten`/`close` could block for the full OS TCP
    /// timeout while Redis is unreachable.
    pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

    fn start_keep_alive_task(
        client: Client,
        connect_timeout: Duration,
        op_lock: Arc<Semaphore>,
        state: Arc<RwLock<ListenState>>,
        keep_alive: Arc<KeepAlive>,
        max_reconnect_backoff: Duration,
    ) {
        tokio::spawn(async move {
            const MIN_BACKOFF: Duration = Duration::from_millis(500);
            // A connection that stayed up at least this long is considered healthy and resets the backoff.
            const STABLE: Duration = Duration::from_secs(10);
            let mut backoff = ExponentialBackoff::new(MIN_BACKOFF, max_reconnect_backoff);

            keep_alive.wait().await;
            while keep_alive.is_running() {
                log::info!("RedisListener reconnection triggered...");

                // Reconnect under the op permit so a concurrent listen() cannot open a second connection.
                let established = {
                    let _permit = op_lock.acquire().await.expect("op_lock is never closed");
                    if !keep_alive.is_running() {
                        break;
                    }
                    Self::connect_and_subscribe(&state, &client, connect_timeout, &keep_alive).await
                };
                match established {
                    Ok(true) => {
                        log::info!("RedisListener reconnected to Redis.");
                        let connected_at = Instant::now();
                        keep_alive.wait().await;
                        if connected_at.elapsed() >= STABLE {
                            backoff.reset();
                        }
                        if keep_alive.is_running() {
                            backoff.delay().await;
                        }
                    }
                    Ok(false) => {
                        // Already connected (listen won the race); park for the next trigger.
                        keep_alive.wait().await;
                    }
                    Err(e) => {
                        log::error!("RedisListener reconnection error: {e:#?}");
                        state.write().await.disconnect();
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
    /// Caller must hold the op permit. The network I/O (connect + subscribe) runs without the
    /// `state` lock and under a timeout, so a Redis outage cannot stall concurrent ops or `close()`.
    async fn connect_and_subscribe(
        state: &Arc<RwLock<ListenState>>,
        client: &Client,
        connect_timeout: Duration,
        keep_alive: &Arc<KeepAlive>,
    ) -> Result<bool, RedisError> {
        if state.read().await.sink.is_some() {
            return Ok(false);
        }

        log::trace!("RedisListener connecting to Redis...");
        let pubsub = with_timeout(connect_timeout, client.get_async_pubsub()).await?;
        let (mut sink, stream) = pubsub.split();

        // The op permit is held, so the handler set can't change while we snapshot and re-subscribe it.
        let channels = state.read().await.handlers.keys().cloned().collect::<Vec<_>>();
        for channel in &channels {
            log::info!("RedisListener subscribing to channel {channel:?}...");
            with_timeout(connect_timeout, sink.subscribe(channel)).await?;
        }

        // Publish the sink and its streaming task only after every re-subscribe succeeded, so a
        // partially-subscribed connection is never left behind for callers to use.
        let task = Self::start_streaming_task(state.clone(), stream, keep_alive.clone());
        {
            let mut state = state.write().await;
            state.set_sink(sink, task);
            state.handle_reconnect();
        }

        Ok(true)
    }

    fn start_streaming_task(
        state: Arc<RwLock<ListenState>>,
        mut stream: PubSubStream,
        keep_alive: Arc<KeepAlive>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                let channel = msg.get_channel_name();
                match msg.get_payload::<String>() {
                    Ok(payload) => state.read().await.handle(channel, Some(&payload)),
                    Err(e) => log::warn!("RedisListener dropping non-UTF-8 payload on channel {channel:?}: {e:#?}"),
                }
            }

            state.write().await.disconnect();

            if keep_alive.is_running() {
                log::info!("RedisListener triggering a reconnection for connection lost...");
                keep_alive.wake();
            }
        })
    }

    pub fn new(client: Client, connect_timeout: Duration, max_reconnect_backoff: Duration) -> Self {
        let keep_alive = Arc::new(KeepAlive::new());
        let op_lock = Arc::new(Semaphore::new(1));
        let state = Arc::new(RwLock::new(ListenState::new()));
        Self::start_keep_alive_task(
            client.clone(),
            connect_timeout,
            op_lock.clone(),
            state.clone(),
            keep_alive.clone(),
            max_reconnect_backoff,
        );

        Self {
            inner: Arc::new(Inner {
                client,
                connect_timeout,
                keep_alive,
                op_lock,
                state,
            }),
        }
    }

    /// Stops the keep-alive task and tears down the shared connection. Sets the stopped flag under
    /// the op permit so a concurrent listen() (which re-checks it under the same permit) can never
    /// leave an unmanaged connection behind.
    pub async fn close(&self) {
        let _permit = self.inner.op_lock.acquire().await.expect("op_lock is never closed");
        self.inner.state.write().await.shutdown();
        self.inner.keep_alive.stop();
    }

    /// Registers `handler` for `channel`, opening the shared connection on first use.
    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), RedisListenerError>
    where
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        let _permit = self.inner.op_lock.acquire().await.expect("op_lock is never closed");

        // Re-check the stopped flag under the permit so a concurrent close() can't leave an
        // unmanaged connection behind.
        if !self.inner.keep_alive.is_running() {
            return Err(RedisListenerError::Closed);
        }
        if self.inner.state.read().await.handlers.contains_key(channel) {
            return Err(RedisListenerError::AlreadyListening(channel.to_string()));
        }

        Self::connect_and_subscribe(
            &self.inner.state,
            &self.inner.client,
            self.inner.connect_timeout,
            &self.inner.keep_alive,
        )
        .await?;

        // Subscribe on a clone of the sink so the network round-trip runs without the state lock. If
        // the connection was just lost, skip it: the handler is registered below and the keep-alive
        // reconnect re-subscribes it.
        let sink = self.inner.state.read().await.sink.clone();
        if let Some(mut sink) = sink {
            log::info!("RedisListener subscribing to channel {channel:?}...");
            with_timeout(self.inner.connect_timeout, sink.subscribe(channel)).await?;
        }
        self.inner
            .state
            .write()
            .await
            .handlers
            .insert(channel.to_string(), Box::new(handler));
        Ok(())
    }

    /// Removes `channel`'s handler and unsubscribes on the shared connection, if connected.
    pub async fn unlisten(&self, channel: &str) -> Result<(), RedisListenerError> {
        let _permit = self.inner.op_lock.acquire().await.expect("op_lock is never closed");
        let sink = {
            let mut state = self.inner.state.write().await;
            if state.handlers.remove(channel).is_none() {
                return Ok(());
            }
            state.sink.clone()
        };
        if let Some(mut sink) = sink {
            log::info!("RedisListener unsubscribing from channel {channel:?}...");
            with_timeout(self.inner.connect_timeout, sink.unsubscribe(channel)).await?;
        }
        Ok(())
    }

    /// Removes all handlers and unsubscribes from every channel on the shared connection.
    pub async fn unlisten_all(&self) -> Result<(), RedisListenerError> {
        let _permit = self.inner.op_lock.acquire().await.expect("op_lock is never closed");
        let (channels, sink) = {
            let mut state = self.inner.state.write().await;
            let channels = state.handlers.drain().map(|(channel, _)| channel).collect::<Vec<_>>();
            (channels, state.sink.clone())
        };
        if let Some(mut sink) = sink {
            // Unsubscribe every channel even if one fails: bailing early would leave the rest
            // subscribed server-side with no local handler. Report the first error afterwards.
            let mut first_err = None;
            for channel in channels {
                log::info!("RedisListener unsubscribing from channel {channel:?}...");
                if let Err(e) = with_timeout(self.inner.connect_timeout, sink.unsubscribe(&channel)).await {
                    first_err = first_err.or(Some(e));
                }
            }
            if let Some(e) = first_err {
                return Err(e.into());
            }
        }
        Ok(())
    }
}

/// Bounds a pub/sub network op; a timeout maps to an `Io` error so the keep-alive task treats it as
/// a lost connection and retries, and `listen`/`unlisten`/`close` never block on a stalled socket
/// for longer than this.
async fn with_timeout<F, T>(timeout: Duration, fut: F) -> Result<T, RedisError>
where
    F: std::future::Future<Output = Result<T, RedisError>>,
{
    match tokio::time::timeout(timeout, fut).await {
        Ok(result) => result,
        Err(_) => Err(RedisError::from((ErrorKind::Io, "Redis pub/sub operation timed out"))),
    }
}
