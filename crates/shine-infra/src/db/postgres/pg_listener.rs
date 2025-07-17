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
type BoxedHandler = Box<dyn Fn(&str) + Send + Sync + 'static>;

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

    async fn connect(&mut self, config: PGConfig, tls: MakeRustlsConnect) -> Result<PGRawSocketConnection, DBError> {
        assert!(self.client.is_none(), "PGListener already connected");

        log::trace!("PGListener connecting to PostgreSQL...");
        let (client, connection) = config.connect(tls).await?;
        log::trace!("PGListener client connected...");

        for channel in self.handlers.keys() {
            log::info!("PGListener start listening to channels {channel:?}...");
            let cmd = format!(r#"LISTEN "{channel}""#);
            client.execute(&cmd, &[]).await?;
            log::info!("PGListener start listening done.");
        }
        self.client = Some(client);

        Ok(connection)
    }

    fn disconnect(&mut self) {
        log::info!("PGListener disconnecting from PostgreSQL...");
        self.client = None;
    }

    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    pub async fn listen<F>(&mut self, channel: &str, handler: F) -> Result<(), DBError>
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let channel = ident(channel);

        if self.handlers.insert(channel.clone(), Box::new(handler)).is_none() {
            if let Some(client) = self.client.as_ref() {
                log::info!("PGListener start listening to channels {channel:?}...");
                let cmd = format!(r#"LISTEN "{channel}""#);
                client.execute(&cmd, &[]).await?;
                log::info!("PGListener start listening done.");
            }
        }

        Ok(())
    }

    pub async fn unlisten(&mut self, channel: &str) -> Result<(), DBError> {
        let channel = ident(channel);

        if self.handlers.remove(&channel).is_some() {
            if let Some(client) = self.client.as_ref() {
                log::info!("PGListener stopping listening to channel {channel}...");
                let cmd = format!(r#"UNLISTEN "{channel}""#);
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

    pub fn handle(&self, channel: &str, payload: &str) {
        if let Some(handler) = self.handlers.get(channel) {
            handler(payload);
        }
    }
}

#[derive(Clone)]
pub struct PGListener {
    config: PGConfig,
    tls: MakeRustlsConnect,
    notify_keep_alive: Arc<(Notify, AtomicBool)>,
    client: Arc<RwLock<ListenClient>>,
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

        tokio::spawn(async move {
            const RETRY: u64 = 500;
            notify_keep_alive.0.notified().await;
            while notify_keep_alive.1.load(Ordering::Relaxed) {
                log::info!("PGListener reconnection triggered...");

                let connection = client.write().await.connect(config.clone(), tls.clone()).await;
                match connection {
                    Ok(connection) => {
                        log::info!("PGListener reconnected to PostgreSQL.");

                        Self::start_streaming_thread(client.clone(), connection, notify_keep_alive.clone());
                        notify_keep_alive.0.notified().await;
                    }
                    Err(e) => {
                        log::error!("PGListener reconnection error: {e:#?}");
                        tokio::time::sleep(tokio::time::Duration::from_millis(RETRY)).await;
                    }
                }
            }
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
                client.handle(msg.channel(), msg.payload());
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
            config,
            tls,
            notify_keep_alive,
            client,
        }
    }

    pub async fn close(&self) {
        self.notify_keep_alive.1.store(false, Ordering::Relaxed);
        self.client.write().await.disconnect();
    }

    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), DBError>
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let mut client = self.client.write().await;

        if !client.is_connected() {
            let connection = client.connect(self.config.clone(), self.tls.clone()).await?;
            Self::start_streaming_thread(self.client.clone(), connection, self.notify_keep_alive.clone());
        }

        client.listen(channel, handler).await?;
        Ok(())
    }

    pub async fn unlisten(&self, channel: &str) -> Result<(), DBError> {
        self.client.write().await.unlisten(channel).await?;
        Ok(())
    }

    /// Stops listening for notifications on all channels.
    pub async fn unlisten_all(&mut self) -> Result<(), DBError> {
        self.client.write().await.unlisten_all().await?;
        Ok(())
    }
}

fn ident(mut name: &str) -> String {
    // If the input string contains a NUL byte, we should truncate the
    // identifier.
    if let Some(index) = name.find('\0') {
        name = &name[..index];
    }

    // Any double quotes must be escaped
    name.replace('"', "\"\"")
}
