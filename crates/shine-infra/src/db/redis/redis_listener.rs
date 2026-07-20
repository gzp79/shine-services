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
use tokio::sync::{Notify, RwLock};

type BoxedHandler = Box<dyn Fn(&str) + Send + Sync + 'static>;

#[derive(Debug, ThisError)]
pub enum RedisListenerError {
    #[error(transparent)]
    Redis(#[from] RedisError),
}

struct ListenState {
    sink: Option<PubSubSink>,
    handlers: HashMap<String, BoxedHandler>,
}

impl ListenState {
    fn new() -> Self {
        Self {
            sink: None,
            handlers: HashMap::new(),
        }
    }

    async fn connect(&mut self, client: &Client) -> Result<PubSubStream, RedisError> {
        assert!(self.sink.is_none(), "RedisListener already connected");

        log::trace!("RedisListener connecting to Redis...");
        let pubsub = client.get_async_pubsub().await?;
        let (mut sink, stream) = pubsub.split();

        for channel in self.handlers.keys() {
            log::info!("RedisListener subscribing to channel {channel:?}...");
            sink.subscribe(channel).await?;
        }
        self.sink = Some(sink);

        Ok(stream)
    }

    fn disconnect(&mut self) {
        log::info!("RedisListener disconnecting from Redis...");
        self.sink = None;
    }

    fn is_connected(&self) -> bool {
        self.sink.is_some()
    }

    async fn listen<F>(&mut self, channel: &str, handler: F) -> Result<(), RedisError>
    where
        F: Fn(&str) + Send + Sync + 'static,
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

    fn handle(&self, channel: &str, payload: &str) {
        if let Some(handler) = self.handlers.get(channel) {
            handler(payload);
        }
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
    client: Client,
    notify_keep_alive: Arc<(Notify, AtomicBool)>,
    state: Arc<RwLock<ListenState>>,
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

        tokio::spawn(async move {
            const RETRY: u64 = 500;
            notify_keep_alive.0.notified().await;
            while notify_keep_alive.1.load(Ordering::Relaxed) {
                log::info!("RedisListener reconnection triggered...");

                let stream = state.write().await.connect(&client).await;
                match stream {
                    Ok(stream) => {
                        log::info!("RedisListener reconnected to Redis.");

                        Self::start_streaming_task(state.clone(), stream, notify_keep_alive.clone());
                        notify_keep_alive.0.notified().await;
                    }
                    Err(err) => {
                        log::error!("RedisListener reconnection error: {err:#?}");
                        tokio::time::sleep(tokio::time::Duration::from_millis(RETRY)).await;
                    }
                }
            }
            log::info!("RedisListener keep alive is closed");
        });
    }

    fn start_streaming_task(
        state: Arc<RwLock<ListenState>>,
        mut stream: PubSubStream,
        notify_keep_alive: Arc<(Notify, AtomicBool)>,
    ) {
        log::trace!("RedisListener starting streaming task...");

        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                let channel = msg.get_channel_name().to_string();
                match msg.get_payload::<String>() {
                    Ok(payload) => {
                        log::trace!("RedisListener received message on channel {channel:?}");
                        state.read().await.handle(&channel, &payload);
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
    }

    pub fn new(raw_cns: &str) -> Result<Self, RedisError> {
        let client = Client::open(raw_cns)?;
        let notify_keep_alive = Arc::new((Notify::new(), AtomicBool::new(true)));
        let state = Arc::new(RwLock::new(ListenState::new()));

        Self::start_keep_alive_task(client.clone(), state.clone(), notify_keep_alive.clone());

        Ok(Self {
            client,
            notify_keep_alive,
            state,
        })
    }

    /// Registers `handler` for `channel`, connecting the shared pub/sub connection on first use.
    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), RedisListenerError>
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let mut state = self.state.write().await;

        if !state.is_connected() {
            let stream = state.connect(&self.client).await?;
            Self::start_streaming_task(self.state.clone(), stream, self.notify_keep_alive.clone());
        }

        state.listen(channel, handler).await?;
        Ok(())
    }

    /// Removes `channel`'s handler and unsubscribes on the shared connection, if connected.
    pub async fn unlisten(&self, channel: &str) -> Result<(), RedisListenerError> {
        self.state.write().await.unlisten(channel).await?;
        Ok(())
    }
}
