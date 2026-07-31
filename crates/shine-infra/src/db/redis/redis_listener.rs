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
};

/// Handler for pub/sub messages.
type BoxedHandler = Box<dyn Fn(Option<&str>) + Send + Sync + 'static>;

#[derive(Debug, ThisError)]
pub enum RedisListenerError {
    #[error(transparent)]
    Redis(#[from] RedisError),
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

    /// Records the handle of the streaming task draining the current connection.
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

    fn handle(&self, channel: &str, payload: Option<&str>) {
        if let Some(handler) = self.handlers.get(channel) {
            handler(payload);
        }
    }

    fn handle_reconnect(&self) {
        for handler in self.handlers.values() {
            handler(None);
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
        // The last RedisListener handle is gone (typically when the pool/AppState is dropped).
        // Signal the keep-alive task to stop and wake it. On exit the task tears down the pub/sub
        // connection (dropping the sink and aborting the streaming task). Without this the
        // keep-alive task and its dedicated Redis connection would live for the whole process
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
    ) {
        // Task to keep the listener connected using notifications. Whenever the connection is
        // (maybe) lost, we trigger a reconnect as long as the listener is not closed. As messages
        // are processed on another task, there's no loop here to detect a connection drop directly
        // — the streaming task notifies this one when its stream ends.
        //
        // The streaming task clears the sink (disconnect) before notifying, so a concurrent
        // `listen()` may reconnect in that window. `connect()` therefore returns `Ok(None)` when a
        // connection already exists and the keep-alive task simply parks, instead of asserting and
        // panicking. This mirrors the race fix in `PGListener`.

        tokio::spawn(async move {
            const RETRY: u64 = 500;
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
                        notify_keep_alive.0.notified().await;
                    }
                    Err(err) => {
                        log::error!("RedisListener reconnection error: {err:#?}");
                        tokio::time::sleep(tokio::time::Duration::from_millis(RETRY)).await;
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

    pub fn new(raw_cns: &str) -> Result<Self, RedisError> {
        let client = Client::open(raw_cns)?;
        let notify_keep_alive = Arc::new((Notify::new(), AtomicBool::new(true)));
        let state = Arc::new(RwLock::new(ListenState::new()));

        Self::start_keep_alive_task(client.clone(), state.clone(), notify_keep_alive.clone());

        Ok(Self {
            inner: Arc::new(Inner {
                client,
                notify_keep_alive,
                state,
            }),
        })
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
        // The write lock is held across connect + set_stream_task + listen, so the keep-alive task
        // and listen() can never create two connections concurrently: whoever takes the lock first
        // connects, the other observes the connection and gets `None`.
        let mut state = self.inner.state.write().await;

        if let Some(stream) = state.connect(&self.inner.client).await? {
            let task =
                Self::start_streaming_task(self.inner.state.clone(), stream, self.inner.notify_keep_alive.clone());
            state.set_stream_task(task);
        }

        state.listen(channel, handler).await?;
        Ok(())
    }

    /// Removes `channel`'s handler and unsubscribes on the shared connection, if connected.
    pub async fn unlisten(&self, channel: &str) -> Result<(), RedisListenerError> {
        self.inner.state.write().await.unlisten(channel).await?;
        Ok(())
    }
}
