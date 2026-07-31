use crate::db::DBError;
use futures::{stream, StreamExt};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{Notify, RwLock};
use tokio_postgres::{AsyncMessage, Notification};
use tokio_postgres_rustls::MakeRustlsConnect;

use super::{PGConfig, PGRawClient, PGRawSocketConnection};

pub type PGNotification = Notification;
type BoxedHandler = Box<dyn Fn(Option<&str>) + Send + Sync + 'static>;

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

        if self.handlers.insert(channel.clone(), Box::new(handler)).is_none() {
            if let Some(client) = self.client.as_ref() {
                Self::pg_listen(client, &channel).await?;
            }
        }

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
            handler(payload);
        }
    }

    pub fn handle_reconnect(&self) {
        for handler in self.handlers.values() {
            handler(None);
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
        // The last PGListener handle is gone (typically when the pool/AppState is dropped).
        // Signal the keep-alive task to stop and wake it. On exit the task disconnects, which
        // drops the LISTEN client and ends the streaming task as well. Without this the keep-alive
        // task and its dedicated PG connection would live for the whole process lifetime.
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
    ) {
        // Task to keep the listener connected using notifications. Whenever the connection is (maybe) lost,
        // we will trigger a reconnect as long as the Pool is not dropped.
        // As the messages are processed using another task, we have no loop on the main "thread" to check for connection lost. When the messaging task
        // detects a connection lost, it will notify the reconnect task to reconnect. As long as the Pool is not dropped, the reconnect task will keep
        // trying to reconnect for each channel.
        // The `client` write lock is held across this call, so `listen()` and the keep-alive task
        // can never create two connections concurrently: whoever takes the lock first connects, the other
        // observes the connection and gets `None`.

        tokio::spawn(async move {
            const RETRY: u64 = 500;
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
                        match client.read().await.relisten().await {
                            Ok(()) => client.read().await.handle_reconnect(),
                            Err(e) => {
                                log::error!("PGListener resubscribe error: {e:#?}");
                                // Drop the just-connected client. This ends the streaming thread we
                                // spawned above (its connection driver completes), and that thread
                                // re-triggers the keep-alive notification on exit. We must NOT loop
                                // back to connect() directly: doing so would spawn a second streaming
                                // thread over a new connection while this one is still polling,
                                // causing duplicate dispatch and an orphaned connection.
                                client.write().await.disconnect();
                                tokio::time::sleep(tokio::time::Duration::from_millis(RETRY)).await;
                            }
                        }
                        // Park until the (single) streaming thread signals a connection loss. On the
                        // relisten-failure path above the notification is already pending, so this
                        // returns promptly and the loop reconnects.
                        notify_keep_alive.0.notified().await;
                    }
                    Err(e) => {
                        log::error!("PGListener reconnection error: {e:#?}");
                        tokio::time::sleep(tokio::time::Duration::from_millis(RETRY)).await;
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

        let messages = stream::poll_fn(move |cx| connection.poll_message(cx)).map(|msg| match msg {
            Ok(AsyncMessage::Notification(notification)) => {
                log::trace!("PGListener received notification: {notification:?}");
                Some(notification)
            }
            Ok(_) => {
                log::trace!("PGListener received no notification");
                None
            }
            Err(e) => {
                log::error!("PGListener notification error: {e:#?}");
                None
            }
        });

        tokio::spawn(async move {
            let mut stream = Box::pin(messages);
            while let Some(Some(msg)) = stream.next().await {
                let client = client.read().await;
                client.handle(msg.channel(), Some(msg.payload()));
            }

            log::trace!("PGListener streaming stopped.");
            client.write().await.disconnect();

            if notify_keep_alive.1.load(Ordering::Relaxed) {
                log::info!("PGListener triggering a reconnection for connection lost...");
                notify_keep_alive.0.notify_one();
            } else {
                log::info!("PGListener is closed, not triggering a reconnect");
            }
        });

        log::trace!("PGListener streaming thread is ready.");
    }

    pub fn new(config: PGConfig, tls: MakeRustlsConnect) -> Self {
        let notify_keep_alive = Arc::new((Notify::new(), AtomicBool::new(true)));
        let client = Arc::new(RwLock::new(ListenClient::new()));
        Self::start_keep_alive_thread(config.clone(), tls.clone(), client.clone(), notify_keep_alive.clone());

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
    }

    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), DBError>
    where
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        let mut client = self.inner.client.write().await;

        if let Some(connection) = client
            .connect(self.inner.config.clone(), self.inner.tls.clone())
            .await?
        {
            Self::start_streaming_thread(
                self.inner.client.clone(),
                connection,
                self.inner.notify_keep_alive.clone(),
            );
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
