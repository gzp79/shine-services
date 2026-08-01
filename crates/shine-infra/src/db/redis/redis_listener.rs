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
    time::{sleep, Duration, Instant},
};

/// Handler for pub/sub messages.
type BoxedHandler = Box<dyn Fn(Option<&str>) + Send + Sync + 'static>;

/// A handler blocking the dispatch task longer than this is logged as a warning.
const SLOW_HANDLER: Duration = Duration::from_millis(100);

#[derive(Debug, ThisError)]
pub enum RedisListenerError {
    #[error(transparent)]
    Redis(#[from] RedisError),
    #[error("The listener has been closed")]
    Closed,
}

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

    /// Connect only if not already connected. Returns the new pub/sub stream to be drained, or
    /// `None` if a connection already exists.
    async fn connect(&mut self, client: &Client) -> Result<Option<PubSubStream>, RedisError> {
        if self.sink.is_some() {
            return Ok(None);
        }

        log::trace!("RedisListener connecting to Redis...");
        let pubsub = client.get_async_pubsub().await?;
        let (mut sink, stream) = pubsub.split();

        for channel in self.handlers.keys() {
            log::info!("RedisListener subscribing to channel {channel:?}...");
            sink.subscribe(channel).await?;
        }
        self.sink = Some(sink);

        Ok(Some(stream))
    }

    fn set_stream_task(&mut self, task: JoinHandle<()>) {
        self.stream_task = Some(task);
    }

    /// Marks the listener disconnected so the next `connect()` re-establishes the connection.
    fn disconnect(&mut self) {
        log::info!("RedisListener disconnecting from Redis...");
        self.sink = None;
    }

    /// Tears down the connection and stops the streaming task, releasing the dedicated connection.
    fn shutdown(&mut self) {
        self.sink = None;
        if let Some(task) = self.stream_task.take() {
            task.abort();
        }
    }

    async fn listen<F>(&mut self, channel: &str, handler: F) -> Result<(), RedisError>
    where
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        if self.handlers.insert(channel.to_string(), Box::new(handler)).is_none() {
            if let Some(sink) = self.sink.as_mut() {
                log::info!("RedisListener subscribing to channel {channel:?}...");
                sink.subscribe(channel).await?;
            }
        }

        Ok(())
    }

    async fn unlisten(&mut self, channel: &str) -> Result<(), RedisError> {
        if self.handlers.remove(channel).is_some() {
            if let Some(sink) = self.sink.as_mut() {
                log::info!("RedisListener unsubscribing from channel {channel:?}...");
                sink.unsubscribe(channel).await?;
            }
        }

        Ok(())
    }

    async fn unlisten_all(&mut self) -> Result<(), RedisError> {
        let channels = self.handlers.drain().map(|(channel, _)| channel).collect::<Vec<_>>();
        if let Some(sink) = self.sink.as_mut() {
            for channel in channels {
                log::info!("RedisListener unsubscribing from channel {channel:?}...");
                sink.unsubscribe(&channel).await?;
            }
        }

        Ok(())
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

    // Handlers run synchronously on the single dispatch task while the read lock is held, so a slow
    // one stalls every other channel and blocks subscription changes. Not queued by design (the
    // caller decides whether to spawn/queue); this just makes an over-long handler visible.
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
    state: Arc<RwLock<ListenState>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Last handle gone: stop the keep-alive task and wake it so it tears down the dedicated
        // pub/sub connection, else the task and its Redis connection would leak for the process
        // lifetime.
        self.notify_keep_alive.1.store(false, Ordering::Relaxed);
        self.notify_keep_alive.0.notify_one();
    }
}

/// Subscribes to Redis pub/sub channels through one shared, reconnecting connection.
/// Mirrors `PGListener`'s role for Postgres `LISTEN`/`NOTIFY`: every channel added via
/// `listen` shares the same underlying `PubSub` connection, split into a `PubSubSink`
/// (subscribe/unsubscribe) and a `PubSubStream` (message reads) so subscriptions can
/// change without blocking dispatch. A keep-alive task reconnects and re-subscribes
/// every channel still registered whenever the connection drops.
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
        // Reconnects whenever the streaming task signals its stream ended, until the listener is
        // closed. A concurrent `listen()` may reconnect in the disconnect-then-notify window, so
        // `connect()` returns `Ok(None)` when already connected and this task simply parks.

        tokio::spawn(async move {
            const RETRY_MIN: Duration = Duration::from_millis(500);
            // A connection that stayed up at least this long is considered healthy and resets the backoff.
            const STABLE: Duration = Duration::from_secs(10);
            let retry_max = max_backoff.max(RETRY_MIN);
            let mut backoff = RETRY_MIN;

            notify_keep_alive.0.notified().await;
            while notify_keep_alive.1.load(Ordering::Relaxed) {
                log::info!("RedisListener reconnection triggered...");

                let stream = state.write().await.connect(&client).await;
                match stream {
                    Ok(None) => {
                        log::info!("RedisListener already connected, skipping reconnect.");
                        notify_keep_alive.0.notified().await;
                    }
                    Ok(Some(stream)) => {
                        log::info!("RedisListener reconnected to Redis.");
                        let task = Self::start_streaming_task(state.clone(), stream, notify_keep_alive.clone());
                        state.write().await.set_stream_task(task);
                        state.read().await.handle_reconnect();
                        let connected_at = Instant::now();
                        notify_keep_alive.0.notified().await;
                        if connected_at.elapsed() >= STABLE {
                            backoff = RETRY_MIN;
                        }
                        if notify_keep_alive.1.load(Ordering::Relaxed) {
                            sleep(backoff).await;
                            backoff = (backoff * 2).min(retry_max);
                        }
                    }
                    Err(err) => {
                        log::error!("RedisListener reconnection error: {err:#?}");
                        sleep(backoff).await;
                        backoff = (backoff * 2).min(retry_max);
                    }
                }
            }
            // Loop exited because the listener was closed (last handle dropped). Tear down the
            // pub/sub connection so the dedicated Redis connection and streaming task are released.
            state.write().await.shutdown();
            log::info!("RedisListener keep alive is closed");
        });
    }

    fn start_streaming_task(
        state: Arc<RwLock<ListenState>>,
        mut stream: PubSubStream,
        notify_keep_alive: Arc<(Notify, AtomicBool)>,
    ) -> JoinHandle<()> {
        log::trace!("RedisListener starting streaming task...");

        let task = tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                let channel = msg.get_channel_name().to_string();
                match msg.get_payload::<String>() {
                    Ok(payload) => {
                        log::trace!("RedisListener received message on channel {channel:?}: [{payload:?}");
                        state.read().await.handle(&channel, Some(&payload));
                    }
                    Err(err) => log::error!("RedisListener payload error on channel {channel:?}: {err:#?}"),
                }
            }

            log::trace!("RedisListener streaming stopped.");
            state.write().await.disconnect();

            if notify_keep_alive.1.load(Ordering::Relaxed) {
                log::info!("RedisListener triggering a reconnection for connection lost...");
                notify_keep_alive.0.notify_one();
            } else {
                log::info!("RedisListener is closed, not triggering a reconnect");
            }
        });

        log::trace!("RedisListener streaming task is ready.");
        task
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

    /// Stops the keep-alive task and tears down the dedicated pub/sub connection.
    pub async fn close(&self) {
        self.inner.notify_keep_alive.1.store(false, Ordering::Relaxed);
        self.inner.state.write().await.shutdown();
        self.inner.notify_keep_alive.0.notify_one();
    }

    /// Registers `handler` for `channel`, connecting the shared pub/sub connection on first use.
    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), RedisListenerError>
    where
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        // A closed listener has no keep-alive task, so a connection opened here would never self-heal
        // on a later drop. Reject rather than resurrect an unmanaged connection.
        if !self.inner.notify_keep_alive.1.load(Ordering::Relaxed) {
            return Err(RedisListenerError::Closed);
        }

        // The write lock is held across connect + set_stream_task + listen, so the keep-alive task
        // and listen() can never create two connections concurrently: whoever takes the lock first
        // connects, the other observes the connection and gets `None`.
        let mut state = self.inner.state.write().await;

        if let Some(stream) = state.connect(&self.inner.client).await? {
            // Fresh connection (this call reconnected before the keep-alive task): notify existing
            // handlers of the bounce, matching the keep-alive path and the PG listener.
            let task =
                Self::start_streaming_task(self.inner.state.clone(), stream, self.inner.notify_keep_alive.clone());
            state.set_stream_task(task);
            state.handle_reconnect();
        }

        state.listen(channel, handler).await?;
        Ok(())
    }

    /// Removes `channel`'s handler and unsubscribes on the shared connection, if connected.
    pub async fn unlisten(&self, channel: &str) -> Result<(), RedisListenerError> {
        self.inner.state.write().await.unlisten(channel).await?;
        Ok(())
    }

    /// Removes all handlers and unsubscribes from every channel on the shared connection.
    pub async fn unlisten_all(&self) -> Result<(), RedisListenerError> {
        self.inner.state.write().await.unlisten_all().await?;
        Ok(())
    }
}
