use crate::{
    db::postgres::PGDBError,
    sync::{ExponentialBackoff, KeepAlive},
};
use futures::{stream, StreamExt};
use std::{collections::HashMap, sync::Arc};
use tokio::{
    sync::{RwLock, Semaphore},
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
    connect_timeout: Duration,
    keep_alive: Arc<KeepAlive>,
    /// Serializes every subscription/lifecycle change; see the design doc, "Concurrency model".
    op_lock: Arc<Semaphore>,
    /// Guards the client handle + handler map. Never held across a network command.
    state: Arc<RwLock<ListenState>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Last handle gone: stop the keep-alive task so it tears the connection down.
        self.keep_alive.stop();
    }
}

#[derive(Clone)]
pub struct PGListener {
    inner: Arc<Inner>,
}

impl PGListener {
    /// Bound for the listener's connect when the connection string sets no `connect_timeout`.
    /// `tokio_postgres`'s own `connect_timeout` covers only the socket connect, not the TLS
    /// handshake/startup, and the keep-alive holds the op permit across the whole connect — so
    /// without this a stalled connect would block `listen`/`unlisten`/`close` on `op_lock.acquire()`.
    pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

    fn start_keep_alive_task(
        config: PGConfig,
        tls: MakeRustlsConnect,
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
                log::info!("PGListener reconnection triggered...");

                // Reconnect under the op permit so a concurrent listen() cannot open a second connection.
                let established = {
                    let _permit = op_lock.acquire().await.expect("op_lock is never closed");
                    if !keep_alive.is_running() {
                        break;
                    }
                    Self::connect_and_subscribe(&config, &tls, connect_timeout, &state, &keep_alive).await
                };

                match established {
                    Ok(true) => {
                        log::info!("PGListener reconnected to PostgreSQL.");
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
    /// Caller must hold the op permit.
    async fn connect_and_subscribe(
        config: &PGConfig,
        tls: &MakeRustlsConnect,
        connect_timeout: Duration,
        state: &Arc<RwLock<ListenState>>,
        keep_alive: &Arc<KeepAlive>,
    ) -> Result<bool, PGDBError> {
        if state.read().await.client.is_some() {
            return Ok(false);
        }

        // Bound the whole connect: tokio_postgres's connect_timeout covers only the socket, so a
        // stall in the TLS handshake or startup would otherwise hold the op permit indefinitely.
        log::trace!("PGListener connecting to PostgreSQL...");
        let (client, connection) = match tokio::time::timeout(connect_timeout, config.connect(tls.clone())).await {
            Ok(result) => result?,
            Err(_) => return Err(PGDBError::ListenerConnectTimeout),
        };
        let client = Arc::new(client);

        // Publish the client and snapshot the channels under one write lock, before any LISTEN: the
        // commands only complete while the driver-polling task polls the connection.
        let channels = {
            let mut state = state.write().await;
            state.set_client(client.clone());
            state.handlers.keys().cloned().collect::<Vec<_>>()
        };
        Self::start_streaming_task(state.clone(), connection, keep_alive.clone());

        for channel in &channels {
            Self::pg_listen(&client, channel).await?;
        }
        state.read().await.handle_reconnect();

        Ok(true)
    }

    fn start_streaming_task(
        state: Arc<RwLock<ListenState>>,
        mut connection: PGRawSocketConnection,
        keep_alive: Arc<KeepAlive>,
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

            if keep_alive.is_running() {
                log::info!("PGListener triggering a reconnection for connection lost...");
                keep_alive.wake();
            }
        });
    }

    pub fn new(
        config: PGConfig,
        tls: MakeRustlsConnect,
        connect_timeout: Duration,
        max_reconnect_backoff: Duration,
    ) -> Self {
        let keep_alive = Arc::new(KeepAlive::new());
        let op_lock = Arc::new(Semaphore::new(1));
        let state = Arc::new(RwLock::new(ListenState::new()));
        Self::start_keep_alive_task(
            config.clone(),
            tls.clone(),
            connect_timeout,
            op_lock.clone(),
            state.clone(),
            keep_alive.clone(),
            max_reconnect_backoff,
        );

        Self {
            inner: Arc::new(Inner {
                config,
                tls,
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
        self.inner.state.write().await.disconnect();
        self.inner.keep_alive.stop();
    }

    /// Registers `handler` for `channel`, opening the shared connection on first use.
    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), PGDBError>
    where
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        let _permit = self.inner.op_lock.acquire().await.expect("op_lock is never closed");

        // Re-check the stopped flag under the permit so a concurrent close() can't leave an
        // unmanaged connection behind.
        if !self.inner.keep_alive.is_running() {
            return Err(PGDBError::ListenerClosed);
        }

        let channel = channel_name(channel);
        if self.inner.state.read().await.handlers.contains_key(&channel) {
            return Err(PGDBError::AlreadyListening(channel));
        }

        Self::connect_and_subscribe(
            &self.inner.config,
            &self.inner.tls,
            self.inner.connect_timeout,
            &self.inner.state,
            &self.inner.keep_alive,
        )
        .await?;

        // Register the handler before returning so the reconnect always re-subscribes it. The LISTEN
        // itself is best-effort: `connect_and_subscribe` reports a client as live via `is_some()`,
        // but the streaming task drops a dead client only just before signalling reconnect, so the
        // snapshot here can be a client whose connection has already died. A failed LISTEN in that
        // window is not the caller's error — the keep-alive reconnect re-subscribes the handler.
        if let Some(client) = self.inner.state.read().await.client.clone() {
            if let Err(e) = Self::pg_listen(&client, &channel).await {
                log::warn!("PGListener LISTEN {channel:?} failed ({e:#?}); reconnect will re-subscribe it");
            }
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
    pub async fn unlisten(&self, channel: &str) -> Result<(), PGDBError> {
        let _permit = self.inner.op_lock.acquire().await.expect("op_lock is never closed");
        let channel = channel_name(channel);

        let client = {
            let mut state = self.inner.state.write().await;
            if state.handlers.remove(&channel).is_none() {
                return Ok(());
            }
            state.client.clone()
        };
        if let Some(client) = client {
            Self::pg_unlisten(&client, &channel).await?;
        }
        Ok(())
    }

    /// Removes all handlers and unsubscribes from every channel on the shared connection.
    pub async fn unlisten_all(&self) -> Result<(), PGDBError> {
        let _permit = self.inner.op_lock.acquire().await.expect("op_lock is never closed");
        let client = {
            let mut state = self.inner.state.write().await;
            state.handlers.clear();
            state.client.clone()
        };
        if let Some(client) = client {
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

    async fn pg_listen(client: &PGRawClient, channel: &str) -> Result<(), PGDBError> {
        log::info!("PGListener start listening to channel {channel:?}...");
        client
            .execute(&format!(r#"LISTEN "{}""#, quote_ident(channel)), &[])
            .await?;
        Ok(())
    }

    async fn pg_unlisten(client: &PGRawClient, channel: &str) -> Result<(), PGDBError> {
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
