use crate::{
    db::postgres::PgError,
    sync::{ExponentialBackoff, KeepAlive},
};
use futures::{stream, StreamExt};
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Weak},
};
use tokio::{
    sync::{Notify, RwLock, Semaphore},
    time::{interval_at, Duration, Instant, Interval, MissedTickBehavior},
};
use tokio_postgres::{AsyncMessage, Notification};
use tokio_postgres_rustls::MakeRustlsConnect;

use super::{PgConfig, PgRawClient, PgRawSocketConnection};

pub type PgNotification = Notification;
type BoxedHandler = Box<dyn Fn(Option<&str>) + Send + Sync + 'static>;

/// A handler blocking the dispatch task longer than this is logged as a warning.
const SLOW_HANDLER: Duration = Duration::from_millis(100);

/// The dedicated `LISTEN` connection (behind an `Arc` so commands can run without the state lock)
/// and the channel→handler map it serves.
struct ListenState {
    client: Option<Arc<PgRawClient>>,
    handlers: HashMap<String, BoxedHandler>,
}

impl ListenState {
    fn new() -> Self {
        Self {
            client: None,
            handlers: HashMap::new(),
        }
    }

    fn set_client(&mut self, client: Arc<PgRawClient>) {
        self.client = Some(client);
    }

    /// Drops the client; its connection driver then ends and the streaming task stops.
    fn disconnect(&mut self) {
        tracing::info!("PGListener disconnecting from PostgreSQL...");
        self.client = None;
    }

    /// Like [`disconnect`](Self::disconnect), but only if `client` is still the published client.
    /// A finished streaming task must not clear a newer connection that replaced its own.
    fn disconnect_if_current(&mut self, client: &Weak<PgRawClient>) -> bool {
        match &self.client {
            Some(current) if Weak::ptr_eq(&Arc::downgrade(current), client) => {
                self.disconnect();
                true
            }
            _ => false,
        }
    }

    fn handle(&self, channel: &str, payload: Option<&str>) {
        if let Some(handler) = self.handlers.get(channel) {
            Self::invoke(channel, handler, payload);
        }
    }

    fn invoke(channel: &str, handler: &BoxedHandler, payload: Option<&str>) {
        let started = Instant::now();
        handler(payload);
        let elapsed = started.elapsed();
        if elapsed >= SLOW_HANDLER {
            tracing::warn!("PGListener handler for channel {channel:?} took {elapsed:?}, blocking dispatch");
        }
    }
}

struct Inner {
    config: PgConfig,
    tls: MakeRustlsConnect,
    connect_timeout: Duration,
    /// Period of the streaming task's handler nudge; `None` disables the periodic wake.
    wake_interval: Option<Duration>,
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
pub struct PgListener {
    inner: Arc<Inner>,
}

impl PgListener {
    /// Bound for the listener's connect when the connection string sets no `connect_timeout`.
    /// `tokio_postgres`'s own `connect_timeout` covers only the socket connect, not the TLS
    /// handshake/startup, and the keep-alive holds the op permit across the whole connect — so
    /// without this a stalled connect would block `listen`/`unlisten`/`close` on `op_lock.acquire()`.
    pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

    fn start_keep_alive_task(
        config: PgConfig,
        tls: MakeRustlsConnect,
        connect_timeout: Duration,
        wake_interval: Option<Duration>,
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
                tracing::info!("PGListener reconnection triggered...");

                // Reconnect under the op permit so a concurrent listen() cannot open a second connection.
                let established = {
                    let _permit = op_lock.acquire().await.expect("op_lock is never closed");
                    if !keep_alive.is_running() {
                        break;
                    }
                    Self::connect_and_subscribe(&config, &tls, connect_timeout, wake_interval, &state, &keep_alive)
                        .await
                };

                match established {
                    Ok(true) => {
                        tracing::info!("PGListener reconnected to PostgreSQL.");
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
                        tracing::error!("PGListener reconnection error: {e:#?}");
                        state.write().await.disconnect();
                        backoff.delay().await;
                    }
                }
            }
            state.write().await.disconnect();
            tracing::info!("PGListener keep alive is closed");
        });
    }

    /// Opens the shared connection if absent, spawns its streaming task, re-subscribes every
    /// registered channel, and triggers the streaming task to nudge their handlers. Returns whether a new connection was opened.
    /// Caller must hold the op permit.
    async fn connect_and_subscribe(
        config: &PgConfig,
        tls: &MakeRustlsConnect,
        connect_timeout: Duration,
        wake_interval: Option<Duration>,
        state: &Arc<RwLock<ListenState>>,
        keep_alive: &Arc<KeepAlive>,
    ) -> Result<bool, PgError> {
        if state.read().await.client.is_some() {
            return Ok(false);
        }

        // Bound the whole connect: tokio_postgres's connect_timeout covers only the socket, so a
        // stall in the TLS handshake or startup would otherwise hold the op permit indefinitely.
        tracing::trace!("PGListener connecting to PostgreSQL...");
        let (client, connection) = match tokio::time::timeout(connect_timeout, config.connect(tls.clone())).await {
            Ok(result) => result?,
            Err(_) => return Err(PgError::ListenerConnectTimeout),
        };
        let client = Arc::new(client);

        // Publish the client and snapshot the channels under one write lock, before any LISTEN: the
        // commands only complete while the driver-polling task polls the connection.
        let channels = {
            let mut state = state.write().await;
            state.set_client(client.clone());
            state.handlers.keys().cloned().collect::<Vec<_>>()
        };
        let wake = Arc::new(Notify::new());
        Self::start_streaming_task(
            state.clone(),
            connection,
            Arc::downgrade(&client),
            keep_alive.clone(),
            wake.clone(),
            wake_interval,
        );

        for channel in &channels {
            Self::pg_listen(&client, channel, connect_timeout).await?;
        }
        // Stored as a permit, so the task picks it up even if it is busy dispatching.
        wake.notify_one();

        Ok(true)
    }

    fn start_streaming_task(
        state: Arc<RwLock<ListenState>>,
        mut connection: PgRawSocketConnection,
        // Weak so the task does not keep the client (and so the driver) alive.
        client: Weak<PgRawClient>,
        keep_alive: Arc<KeepAlive>,
        wake: Arc<Notify>,
        wake_interval: Option<Duration>,
    ) {
        // Raw poll result so the loop can tell a notification (dispatch) from a non-notification
        // async message such as a server NOTICE (skip, keep polling) from an error (stop, reconnect).
        let messages = stream::poll_fn(move |cx| connection.poll_message(cx));

        tokio::spawn(async move {
            let mut stream = Box::pin(messages);
            let mut ticker = wake_interval.filter(|d| !d.is_zero()).map(|period| {
                let mut ticker = interval_at(Instant::now() + period, period);
                ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
                ticker
            });

            loop {
                tokio::select! {
                    msg = stream.next() => match msg {
                        Some(Ok(AsyncMessage::Notification(notification))) => {
                            state
                                .read()
                                .await
                                .handle(notification.channel(), Some(notification.payload()));
                        }
                        Some(Ok(_)) => {}
                        Some(Err(e)) => {
                            tracing::error!("PGListener notification error: {e:#?}");
                            break;
                        }
                        None => break,
                    },

                    // Woken by a (re)connect or the periodic tick: nudge every handler without a payload.
                    _ = wake_or_tick(&wake, &mut ticker) => {
                        let state = state.read().await;
                        for channel in state.handlers.keys() {
                            state.handle(channel, None);
                        }
                    }
                }
            }

            // End the driver before clearing the client. Only a still-current client is cleared and
            // triggers a reconnect: otherwise the keep-alive task already dropped it (and is retrying)
            // or a newer connection replaced it, which this task must not tear down.
            drop(stream);
            let was_current = state.write().await.disconnect_if_current(&client);

            if was_current && keep_alive.is_running() {
                tracing::info!("PGListener triggering a reconnection for connection lost...");
                keep_alive.wake();
            }
        });
    }

    pub fn new(
        config: PgConfig,
        tls: MakeRustlsConnect,
        connect_timeout: Duration,
        wake_interval: Option<Duration>,
        max_reconnect_backoff: Duration,
    ) -> Self {
        let keep_alive = Arc::new(KeepAlive::new());
        let op_lock = Arc::new(Semaphore::new(1));
        let state = Arc::new(RwLock::new(ListenState::new()));
        Self::start_keep_alive_task(
            config.clone(),
            tls.clone(),
            connect_timeout,
            wake_interval,
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
                wake_interval,
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
    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), PgError>
    where
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        let _permit = self.inner.op_lock.acquire().await.expect("op_lock is never closed");

        // Re-check the stopped flag under the permit so a concurrent close() can't leave an
        // unmanaged connection behind.
        if !self.inner.keep_alive.is_running() {
            return Err(PgError::ListenerClosed);
        }

        let channel = channel_name(channel);
        if self.inner.state.read().await.handlers.contains_key(&channel) {
            return Err(PgError::AlreadyListening(channel));
        }

        Self::connect_and_subscribe(
            &self.inner.config,
            &self.inner.tls,
            self.inner.connect_timeout,
            self.inner.wake_interval,
            &self.inner.state,
            &self.inner.keep_alive,
        )
        .await?;

        self.inner
            .state
            .write()
            .await
            .handlers
            .insert(channel.clone(), Box::new(handler));
        let client = self.inner.state.read().await.client.clone();
        if let Some(client) = client {
            if let Err(e) = Self::pg_listen(&client, &channel, self.inner.connect_timeout).await {
                self.inner.state.write().await.handlers.remove(&channel);
                return Err(e);
            }
        }
        Ok(())
    }

    /// Removes `channel`'s handler and unsubscribes on the shared connection, if connected.
    pub async fn unlisten(&self, channel: &str) -> Result<(), PgError> {
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
            Self::pg_unlisten(&client, &channel, self.inner.connect_timeout).await?;
        }
        Ok(())
    }

    /// Removes all handlers and unsubscribes from every channel on the shared connection.
    pub async fn unlisten_all(&self) -> Result<(), PgError> {
        let _permit = self.inner.op_lock.acquire().await.expect("op_lock is never closed");
        let client = {
            let mut state = self.inner.state.write().await;
            state.handlers.clear();
            state.client.clone()
        };
        if let Some(client) = client {
            tracing::info!("PGListener stop listening to all channels...");
            with_timeout(self.inner.connect_timeout, client.execute("UNLISTEN *", &[])).await?;
        }
        Ok(())
    }

    pub async fn backend_pid(&self) -> Option<i32> {
        let client = self.inner.state.read().await.client.clone()?;
        with_timeout(
            self.inner.connect_timeout,
            client.query_one("SELECT pg_backend_pid()::int4", &[]),
        )
        .await
        .ok()
        .map(|row| row.get(0))
    }

    async fn pg_listen(client: &PgRawClient, channel: &str, timeout: Duration) -> Result<(), PgError> {
        tracing::info!("PGListener start listening to channel {channel:?}...");
        with_timeout(
            timeout,
            client.execute(&format!(r#"LISTEN "{}""#, quote_ident(channel)), &[]),
        )
        .await?;
        Ok(())
    }

    async fn pg_unlisten(client: &PgRawClient, channel: &str, timeout: Duration) -> Result<(), PgError> {
        tracing::info!("PGListener stop listening to channel {channel:?}...");
        with_timeout(
            timeout,
            client.execute(&format!(r#"UNLISTEN "{}""#, quote_ident(channel)), &[]),
        )
        .await?;
        Ok(())
    }
}

/// Bounds a command on the listener connection. A stalled socket would otherwise hold the op permit
/// and block `listen`/`unlisten`/`close` and the reconnect; a timeout is reported like a failed connect.
async fn with_timeout<F, T, E>(timeout: Duration, fut: F) -> Result<T, PgError>
where
    F: Future<Output = Result<T, E>>,
    PgError: From<E>,
{
    match tokio::time::timeout(timeout, fut).await {
        Ok(result) => Ok(result?),
        Err(_) => Err(PgError::ListenerConnectTimeout),
    }
}

/// Resolves on an explicit wake or the next periodic tick (if enabled).
async fn wake_or_tick(wake: &Notify, ticker: &mut Option<Interval>) {
    match ticker {
        Some(ticker) => tokio::select! {
            _ = wake.notified() => {}
            _ = ticker.tick() => {}
        },
        None => wake.notified().await,
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
