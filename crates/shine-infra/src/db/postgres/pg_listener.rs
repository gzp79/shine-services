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
    sync::{Notify, RwLock},
    time::{Duration, Instant},
};
use tokio_postgres::{AsyncMessage, Notification};
use tokio_postgres_rustls::MakeRustlsConnect;

use super::{PGConfig, PGRawClient, PGRawSocketConnection};

pub type PGNotification = Notification;
type BoxedHandler = Box<dyn Fn(Option<&str>) + Send + Sync + 'static>;

/// A handler blocking the dispatch task longer than this is logged as a warning.
const SLOW_HANDLER: Duration = Duration::from_millis(100);

struct ListenClient {
    client: Option<PGRawClient>,
    handlers: HashMap<String, BoxedHandler>,
}

impl ListenClient {
    fn new() -> Self {
        Self {
            client: None,
            handlers: HashMap::new(),
        }
    }

    /// Connect only if not already connected. Returns the new socket connection to be streamed, or
    /// `None` if a connection already exists.
    async fn connect(
        &mut self,
        config: PGConfig,
        tls: MakeRustlsConnect,
    ) -> Result<Option<PGRawSocketConnection>, DBError> {
        if self.client.is_some() {
            return Ok(None);
        }

        log::trace!("PGListener connecting to PostgreSQL...");
        let (client, connection) = config.connect(tls).await?;
        log::trace!("PGListener client connected...");

        self.client = Some(client);

        Ok(Some(connection))
    }

    fn disconnect(&mut self) {
        log::info!("PGListener disconnecting from PostgreSQL...");
        self.client = None;
    }

    pub async fn listen<F>(&mut self, channel: &str, handler: F) -> Result<(), DBError>
    where
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        let channel = channel_name(channel);

        if self.handlers.contains_key(&channel) {
            return Err(DBError::AlreadyListening(channel));
        }
        if let Some(client) = self.client.as_ref() {
            Self::pg_listen(client, &channel).await?;
        }
        self.handlers.insert(channel, Box::new(handler));

        Ok(())
    }

    // Must be called after the connection driver is spawned; re-sends LISTEN for all registered channels.
    pub async fn relisten(&self) -> Result<(), DBError> {
        if let Some(client) = &self.client {
            for channel in self.handlers.keys() {
                Self::pg_listen(client, channel).await?;
            }
        }
        Ok(())
    }

    async fn pg_listen(client: &PGRawClient, channel: &str) -> Result<(), DBError> {
        log::info!("PGListener start listening to channel {channel:?}...");
        client
            .execute(&format!(r#"LISTEN "{}""#, quote_ident(channel)), &[])
            .await?;
        Ok(())
    }

    pub async fn unlisten(&mut self, channel: &str) -> Result<(), DBError> {
        let channel = channel_name(channel);

        if self.handlers.remove(&channel).is_some() {
            if let Some(client) = self.client.as_ref() {
                log::info!("PGListener stopping listening to channel {channel}...");
                let cmd = format!(r#"UNLISTEN "{}""#, quote_ident(&channel));
                client.execute(&cmd, &[]).await?;
                log::info!("PGListener stopped listening");
            }
        }

        Ok(())
    }

    pub async fn unlisten_all(&mut self) -> Result<(), DBError> {
        self.handlers.clear();
        if let Some(client) = self.client.as_ref() {
            let cmd = "UNLISTEN *".to_string();
            client.execute(&cmd, &[]).await?;
            log::info!("PGListener stopped listening to all channels");
        }
        Ok(())
    }

    pub fn handle(&self, channel: &str, payload: Option<&str>) {
        if let Some(handler) = self.handlers.get(channel) {
            Self::invoke(channel, handler, payload);
        }
    }

    pub fn handle_reconnect(&self) {
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
            log::warn!("PGListener handler for channel {channel:?} took {elapsed:?}, blocking dispatch");
        }
    }
}

struct Inner {
    config: PGConfig,
    tls: MakeRustlsConnect,
    notify_keep_alive: Arc<(Notify, AtomicBool)>,
    client: Arc<RwLock<ListenClient>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Last handle gone: stop the keep-alive task and wake it so it tears down the dedicated
        // connection, else the task and its PG connection would leak for the process lifetime.
        self.notify_keep_alive.1.store(false, Ordering::Relaxed);
        self.notify_keep_alive.0.notify_one();
    }
}

#[derive(Clone)]
pub struct PGListener {
    inner: Arc<Inner>,
}

impl PGListener {
    fn start_keep_alive_thread(
        config: PGConfig,
        tls: MakeRustlsConnect,
        client: Arc<RwLock<ListenClient>>,
        notify_keep_alive: Arc<(Notify, AtomicBool)>,
        max_backoff: Duration,
    ) {
        // Reconnects whenever the streaming thread signals a connection loss, until the pool is
        // dropped. The `client` write lock serializes this task and `listen()`, so the two can never
        // open two connections concurrently: the loser observes the connection and gets `None`.

        tokio::spawn(async move {
            const RETRY_MIN: Duration = Duration::from_millis(500);
            // A connection that stayed up at least this long is considered healthy and resets the backoff.
            const STABLE: Duration = Duration::from_secs(10);
            let mut backoff = ExponentialBackoff::new(RETRY_MIN, max_backoff);

            notify_keep_alive.0.notified().await;
            while notify_keep_alive.1.load(Ordering::Relaxed) {
                log::info!("PGListener reconnection triggered...");

                let connection = client.write().await.connect(config.clone(), tls.clone()).await;
                match connection {
                    Ok(None) => {
                        // Another caller (listen()) connected first and already started a streaming
                        // thread. Park until that connection is lost, then reconnect.
                        log::info!("PGListener already connected, skipping reconnect.");
                        notify_keep_alive.0.notified().await;
                    }
                    Ok(Some(connection)) => {
                        log::info!("PGListener reconnected to PostgreSQL.");

                        Self::start_streaming_thread(client.clone(), connection, notify_keep_alive.clone());
                        let connected_at = Instant::now();
                        match client.read().await.relisten().await {
                            Ok(()) => client.read().await.handle_reconnect(),
                            Err(e) => {
                                log::error!("PGListener resubscribe error: {e:#?}");
                                // Must not loop straight back to connect(): that would spawn a second
                                // streaming thread over the still-polling connection. disconnect()
                                // ends this thread, which re-triggers the notification below.
                                client.write().await.disconnect();
                            }
                        }
                        notify_keep_alive.0.notified().await;
                        // Reset only after a connection that stayed up, so accept-then-drop keeps backing off.
                        if connected_at.elapsed() >= STABLE {
                            backoff.reset();
                        }
                        if notify_keep_alive.1.load(Ordering::Relaxed) {
                            backoff.delay().await;
                        }
                    }
                    Err(e) => {
                        log::error!("PGListener reconnection error: {e:#?}");
                        backoff.delay().await;
                    }
                }
            }
            // Loop exited because the listener was closed (last handle dropped). Drop the LISTEN
            // client so the dedicated PG connection is torn down and the streaming task ends.
            client.write().await.disconnect();
            log::info!("PGListener keep alive is closed");
        });
    }

    fn start_streaming_thread(
        client: Arc<RwLock<ListenClient>>,
        mut connection: PGRawSocketConnection,
        notify_keep_alive: Arc<(Notify, AtomicBool)>,
    ) {
        log::trace!("PGListener starting streaming thread...");

        // Yields the raw poll result so the streaming loop can distinguish a notification (dispatch),
        // a non-notification async message such as a server NOTICE (skip and keep polling), and an
        // error (stop and let the keep-alive task reconnect).
        let messages = stream::poll_fn(move |cx| connection.poll_message(cx));

        tokio::spawn(async move {
            let mut stream = Box::pin(messages);
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(AsyncMessage::Notification(notification)) => {
                        log::trace!("PGListener received notification: {notification:?}");
                        let client = client.read().await;
                        client.handle(notification.channel(), Some(notification.payload()));
                    }
                    Ok(_) => {
                        // Non-notification async message (e.g. a server NOTICE). Skip it and keep
                        // polling; it must NOT tear the connection down and force a reconnect.
                        log::trace!("PGListener received no notification");
                    }
                    Err(e) => {
                        // A real connection/driver error: stop streaming so the keep-alive task
                        // reconnects.
                        log::error!("PGListener notification error: {e:#?}");
                        break;
                    }
                }
            }

            log::trace!("PGListener stopping stream...");
            // Drop the connection driver before taking the write lock. A query still in flight on
            // this client (e.g. backend_pid) holds a read lock while awaiting a response only this
            // now-stopped driver could deliver; dropping it makes that query fail fast, releasing
            // the read lock so disconnect() below proceeds instead of deadlocking.
            drop(stream);
            client.write().await.disconnect();
            log::trace!("PGListener streaming stopped.");

            if notify_keep_alive.1.load(Ordering::Relaxed) {
                log::info!("PGListener triggering a reconnection for connection lost...");
                notify_keep_alive.0.notify_one();
            } else {
                log::info!("PGListener is closed, not triggering a reconnect");
            }
        });

        log::trace!("PGListener streaming thread is ready.");
    }

    pub fn new(config: PGConfig, tls: MakeRustlsConnect, max_backoff: Duration) -> Self {
        let notify_keep_alive = Arc::new((Notify::new(), AtomicBool::new(true)));
        let client = Arc::new(RwLock::new(ListenClient::new()));
        Self::start_keep_alive_thread(
            config.clone(),
            tls.clone(),
            client.clone(),
            notify_keep_alive.clone(),
            max_backoff,
        );

        Self {
            inner: Arc::new(Inner {
                config,
                tls,
                notify_keep_alive,
                client,
            }),
        }
    }

    pub async fn close(&self) {
        self.inner.notify_keep_alive.1.store(false, Ordering::Relaxed);
        self.inner.client.write().await.disconnect();
        // Wake the keep-alive task so it observes the cleared flag and exits now, instead of parking
        // until the last handle drops.
        self.inner.notify_keep_alive.0.notify_one();
    }

    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), DBError>
    where
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        // A closed listener has no keep-alive task, so a connection opened here would never self-heal
        // on a later drop. Reject rather than resurrect an unmanaged connection.
        if !self.inner.notify_keep_alive.1.load(Ordering::Relaxed) {
            return Err(DBError::ListenerClosed);
        }

        let mut client = self.inner.client.write().await;

        if let Some(connection) = client
            .connect(self.inner.config.clone(), self.inner.tls.clone())
            .await?
        {
            // Fresh connection (this call reconnected before the keep-alive task): re-subscribe all
            // already-registered channels, else only the new one below would be LISTENed.
            Self::start_streaming_thread(
                self.inner.client.clone(),
                connection,
                self.inner.notify_keep_alive.clone(),
            );
            client.relisten().await?;
            client.handle_reconnect();
        }

        client.listen(channel, handler).await?;
        Ok(())
    }

    pub async fn unlisten(&self, channel: &str) -> Result<(), DBError> {
        self.inner.client.write().await.unlisten(channel).await?;
        Ok(())
    }

    /// Stops listening for notifications on all channels.
    pub async fn unlisten_all(&self) -> Result<(), DBError> {
        self.inner.client.write().await.unlisten_all().await?;
        Ok(())
    }

    pub async fn backend_pid(&self) -> Option<i32> {
        let client = self.inner.client.read().await;
        if let Some(pg_client) = &client.client {
            pg_client
                .query_one("SELECT pg_backend_pid()::int4", &[])
                .await
                .ok()
                .map(|row| row.get(0))
        } else {
            None
        }
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
