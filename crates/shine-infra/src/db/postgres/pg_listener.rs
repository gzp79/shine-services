use crate::{db::DBError, sync::ExponentialBackoff};
use futures::{stream, StreamExt};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    sync::{Notify, RwLock, Semaphore},
    time::{Duration, Instant},
};
use tokio_postgres::{AsyncMessage, Notification};
use tokio_postgres_rustls::MakeRustlsConnect;

use super::{PGConfig, PGRawClient, PGRawSocketConnection};

pub type PGNotification = Notification;
type BoxedHandler = Box<dyn Fn(Option<&str>) + Send + Sync + 'static>;

/// A handler blocking the dispatch task longer than this is logged as a warning.
const SLOW_HANDLER: Duration = Duration::from_millis(100);

/// The dedicated `LISTEN` connection (behind an `Arc` so commands can run without the state lock)
/// and the channel→handler map it serves.
struct ListenState {
    client: Option<Arc<PGRawClient>>,
    handlers: HashMap<String, BoxedHandler>,
}

impl ListenState {
    fn new() -> Self {
        Self {
            client: None,
            handlers: HashMap::new(),
        }
    }

    fn set_client(&mut self, client: Arc<PGRawClient>) {
        self.client = Some(client);
    }

    /// Drops the client; its connection driver then ends and the streaming task stops.
    fn disconnect(&mut self) {
        log::info!("PGListener disconnecting from PostgreSQL...");
        self.client = None;
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
            log::warn!("PGListener handler for channel {channel:?} took {elapsed:?}, blocking dispatch");
        }
    }
}

struct Inner {
    config: PGConfig,
    tls: MakeRustlsConnect,
    notify_keep_alive: Arc<(Notify, AtomicBool)>,
    /// Serializes every subscription/lifecycle change; see the design doc, "Concurrency model".
    ops: Arc<Semaphore>,
    /// Guards the client handle + handler map. Never held across a network command.
    state: Arc<RwLock<ListenState>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Last handle gone: stop the keep-alive task and wake it so it tears the connection down.
        self.notify_keep_alive.1.store(false, Ordering::Relaxed);
        self.notify_keep_alive.0.notify_one();
    }
}

#[derive(Clone)]
pub struct PGListener {
    inner: Arc<Inner>,
}

impl PGListener {
    fn start_keep_alive_task(
        config: PGConfig,
        tls: MakeRustlsConnect,
        ops: Arc<Semaphore>,
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
                log::info!("PGListener reconnection triggered...");

                // Reconnect under the ops permit so a concurrent listen() cannot open a second connection.
                let established = {
                    let _permit = ops.acquire().await.expect("ops semaphore is never closed");
                    if !notify_keep_alive.1.load(Ordering::Relaxed) {
                        break;
                    }
                    Self::connect_and_subscribe(&config, &tls, &state, &notify_keep_alive).await
                };

                match established {
                    Ok(true) => {
                        log::info!("PGListener reconnected to PostgreSQL.");
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
                    Err(e) => {
                        log::error!("PGListener reconnection error: {e:#?}");
                        state.write().await.disconnect();
                        backoff.delay().await;
                    }
                }
            }
            state.write().await.disconnect();
            log::info!("PGListener keep alive is closed");
        });
    }

    /// Opens the shared connection if absent, spawns its streaming task, re-subscribes every
    /// registered channel, and nudges their handlers. Returns whether a new connection was opened.
    /// Caller must hold the ops permit.
    async fn connect_and_subscribe(
        config: &PGConfig,
        tls: &MakeRustlsConnect,
        state: &Arc<RwLock<ListenState>>,
        notify_keep_alive: &Arc<(Notify, AtomicBool)>,
    ) -> Result<bool, DBError> {
        if state.read().await.client.is_some() {
            return Ok(false);
        }

        log::trace!("PGListener connecting to PostgreSQL...");
        let (client, connection) = config.connect(tls.clone()).await?;
        let client = Arc::new(client);

        // Publish the client and spawn the driver-polling task before any LISTEN: the commands only
        // complete while that task polls the connection.
        state.write().await.set_client(client.clone());
        Self::start_streaming_task(state.clone(), connection, notify_keep_alive.clone());

        let channels = state.read().await.handlers.keys().cloned().collect::<Vec<_>>();
        for channel in &channels {
            Self::pg_listen(&client, channel).await?;
        }
        state.read().await.handle_reconnect();

        Ok(true)
    }

    fn start_streaming_task(
        state: Arc<RwLock<ListenState>>,
        mut connection: PGRawSocketConnection,
        notify_keep_alive: Arc<(Notify, AtomicBool)>,
    ) {
        // Raw poll result so the loop can tell a notification (dispatch) from a non-notification
        // async message such as a server NOTICE (skip, keep polling) from an error (stop, reconnect).
        let messages = stream::poll_fn(move |cx| connection.poll_message(cx));

        tokio::spawn(async move {
            let mut stream = Box::pin(messages);
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(AsyncMessage::Notification(notification)) => {
                        state
                            .read()
                            .await
                            .handle(notification.channel(), Some(notification.payload()));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("PGListener notification error: {e:#?}");
                        break;
                    }
                }
            }

            // End the driver before clearing the client. This runs before the reconnect notify, so
            // the keep-alive task cannot have opened a replacement yet.
            drop(stream);
            state.write().await.disconnect();

            if notify_keep_alive.1.load(Ordering::Relaxed) {
                log::info!("PGListener triggering a reconnection for connection lost...");
                notify_keep_alive.0.notify_one();
            }
        });
    }

    pub fn new(config: PGConfig, tls: MakeRustlsConnect, max_backoff: Duration) -> Self {
        let notify_keep_alive = Arc::new((Notify::new(), AtomicBool::new(true)));
        let ops = Arc::new(Semaphore::new(1));
        let state = Arc::new(RwLock::new(ListenState::new()));
        Self::start_keep_alive_task(
            config.clone(),
            tls.clone(),
            ops.clone(),
            state.clone(),
            notify_keep_alive.clone(),
            max_backoff,
        );

        Self {
            inner: Arc::new(Inner {
                config,
                tls,
                notify_keep_alive,
                ops,
                state,
            }),
        }
    }

    /// Stops the keep-alive task and tears down the shared connection.
    pub async fn close(&self) {
        let _permit = self.inner.ops.acquire().await.expect("ops semaphore is never closed");
        self.inner.notify_keep_alive.1.store(false, Ordering::Relaxed);
        self.inner.state.write().await.disconnect();
        self.inner.notify_keep_alive.0.notify_one();
    }

    /// Registers `handler` for `channel`, opening the shared connection on first use.
    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), DBError>
    where
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        let _permit = self.inner.ops.acquire().await.expect("ops semaphore is never closed");

        // Re-check the closed flag under the permit so a concurrent close() can't leave an
        // unmanaged connection behind.
        if !self.inner.notify_keep_alive.1.load(Ordering::Relaxed) {
            return Err(DBError::ListenerClosed);
        }

        let channel = channel_name(channel);
        if self.inner.state.read().await.handlers.contains_key(&channel) {
            return Err(DBError::AlreadyListening(channel));
        }

        Self::connect_and_subscribe(
            &self.inner.config,
            &self.inner.tls,
            &self.inner.state,
            &self.inner.notify_keep_alive,
        )
        .await?;

        // If the connection was just lost, skip the command: the handler is registered below and the
        // keep-alive reconnect re-subscribes it.
        if let Some(client) = self.inner.state.read().await.client.clone() {
            Self::pg_listen(&client, &channel).await?;
        }
        self.inner
            .state
            .write()
            .await
            .handlers
            .insert(channel, Box::new(handler));
        Ok(())
    }

    /// Removes `channel`'s handler and unsubscribes on the shared connection, if connected.
    pub async fn unlisten(&self, channel: &str) -> Result<(), DBError> {
        let _permit = self.inner.ops.acquire().await.expect("ops semaphore is never closed");
        let channel = channel_name(channel);

        if self.inner.state.write().await.handlers.remove(&channel).is_none() {
            return Ok(());
        }
        if let Some(client) = self.inner.state.read().await.client.clone() {
            Self::pg_unlisten(&client, &channel).await?;
        }
        Ok(())
    }

    /// Removes all handlers and unsubscribes from every channel on the shared connection.
    pub async fn unlisten_all(&self) -> Result<(), DBError> {
        let _permit = self.inner.ops.acquire().await.expect("ops semaphore is never closed");
        self.inner.state.write().await.handlers.clear();
        if let Some(client) = self.inner.state.read().await.client.clone() {
            log::info!("PGListener stop listening to all channels...");
            client.execute("UNLISTEN *", &[]).await?;
        }
        Ok(())
    }

    pub async fn backend_pid(&self) -> Option<i32> {
        let client = self.inner.state.read().await.client.clone()?;
        client
            .query_one("SELECT pg_backend_pid()::int4", &[])
            .await
            .ok()
            .map(|row| row.get(0))
    }

    async fn pg_listen(client: &PGRawClient, channel: &str) -> Result<(), DBError> {
        log::info!("PGListener start listening to channel {channel:?}...");
        client
            .execute(&format!(r#"LISTEN "{}""#, quote_ident(channel)), &[])
            .await?;
        Ok(())
    }

    async fn pg_unlisten(client: &PGRawClient, channel: &str) -> Result<(), DBError> {
        log::info!("PGListener stop listening to channel {channel:?}...");
        client
            .execute(&format!(r#"UNLISTEN "{}""#, quote_ident(channel)), &[])
            .await?;
        Ok(())
    }
}

/// PostgreSQL identifier length limit (NAMEDATALEN - 1). Channel names longer than this are
/// truncated by the server, so `msg.channel()` reports the truncated form.
const PG_NAMEDATALEN: usize = 63;

/// Canonicalize a channel name to exactly what PostgreSQL will report back in a notification.
fn channel_name(mut name: &str) -> String {
    if let Some(index) = name.find('\0') {
        name = &name[..index];
    }

    if name.len() > PG_NAMEDATALEN {
        let mut end = PG_NAMEDATALEN;
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        name = &name[..end];
    }

    name.to_string()
}

/// Escape a canonical channel name for use inside a double-quoted SQL identifier.
fn quote_ident(name: &str) -> String {
    name.replace('"', "\"\"")
}
