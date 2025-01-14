use crate::db::DBError;
use futures::{stream, StreamExt};
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio_postgres::{AsyncMessage, Notification};
use tokio_postgres_rustls::MakeRustlsConnect;

use super::{PGConfig, PGRawClient};

pub type PGNotification = Notification;

type PGNotificationHandlers = Arc<RwLock<HashMap<String, Box<dyn Fn(&str) + Send + Sync + 'static>>>>;

struct Inner {
    config: PGConfig,
    tls: MakeRustlsConnect,
    client: Option<PGRawClient>,
    handlers: PGNotificationHandlers,
    connect_lost: Arc<(Notify, AtomicBool)>,
}

impl Inner {
    pub fn new(config: PGConfig, tls: MakeRustlsConnect, connect_lost: Arc<(Notify, AtomicBool)>) -> Self {
        Self {
            config: config.clone(),
            tls: tls.clone(),
            client: None,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            connect_lost,
        }
    }

    async fn message_stream(&mut self) -> Result<(), DBError> {
        let (client, mut connection) = self.config.connect(self.tls.clone()).await?;

        self.client = Some(client);

        let messages = stream::poll_fn(move |cx| connection.poll_message(cx)).filter_map(|msg| async move {
            match msg {
                Ok(AsyncMessage::Notification(notification)) => Some(notification),
                Ok(_) => None,
                Err(e) => {
                    log::error!("PGListener notification error: {:#?}", e);
                    None
                }
            }
        });

        let handlers = self.handlers.clone();
        let close_notifier = self.connect_lost.clone();

        tokio::spawn(async move {
            let mut stream = Box::pin(messages);
            while let Some(msg) = stream.next().await {
                let handlers = handlers.read().await;
                if let Some(handler) = handlers.get(msg.channel()) {
                    handler(msg.payload());
                }
            }

            if !close_notifier.1.load(Ordering::Relaxed) {
                log::info!("PGListener triggering a reconnection for connection lost...");
                close_notifier.0.notify_one();
            } else {
                log::info!("PGListener is closed, not triggering a reconnect");
            }
        });

        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    pub async fn connect(&mut self) -> Result<(), DBError> {
        if self.client.is_some() {
            return Ok(());
        }

        self.message_stream().await?;

        // re-listen to all channels
        for channel in self.handlers.read().await.keys() {
            let cmd = format!(r#"LISTEN "{}""#, channel);
            self.client.as_ref().unwrap().execute(&cmd, &[]).await?;
        }

        Ok(())
    }

    pub async fn listen<F>(&mut self, channel: &str, handler: F) -> Result<(), DBError>
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let ch = ident(channel);

        let mut handlers = self.handlers.write().await;
        let handlers = &mut *handlers;

        match handlers.entry(ch) {
            Entry::Occupied(entry) => {
                *entry.into_mut() = Box::new(handler);
            }
            Entry::Vacant(entry) => {
                let cmd = format!(r#"LISTEN "{}""#, entry.key());
                entry.insert(Box::new(handler));
                // if there is no client (the connection is lost), it will be reconnect and listener will be re-added
                if let Some(client) = self.client.as_ref() {
                    client.execute(&cmd, &[]).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn unlisten(&mut self, channel: &str) -> Result<(), DBError> {
        let ch = ident(channel);

        let mut handlers = self.handlers.write().await;
        let handlers = &mut *handlers;

        if handlers.remove(&ch).is_some() {
            let cmd = format!(r#"UNLISTEN "{}""#, ch);
            // if there is no client (the connection is lost), thus no need to unlisten
            if let Some(client) = self.client.as_ref() {
                client.execute(&cmd, &[]).await?;
            }
        }

        Ok(())
    }

    /// Stops listening for notifications on all channels.
    pub async fn unlisten_all(&mut self) -> Result<(), DBError> {
        let mut handlers = self.handlers.write().await;
        let handlers = &mut *handlers;

        // if there is no client (the connection is lost), thus no need to unlisten
        if let Some(client) = self.client.as_ref() {
            client.execute("UNLISTEN *", &[]).await?;
        }
        handlers.clear();

        Ok(())
    }
}

#[derive(Clone)]
pub struct PGListener {
    notify: Arc<(Notify, AtomicBool)>,
    inner: Arc<Mutex<Inner>>,
}

impl PGListener {
    pub fn new(config: PGConfig, tls: MakeRustlsConnect) -> Self {
        let notify = Arc::new((Notify::new(), AtomicBool::new(false)));
        let inner = Arc::new(Mutex::new(Inner::new(config, tls, notify.clone())));

        // Task to keep the listener connected using notifications. Whenever the connection is (maybe) lost,
        // we will trigger a reconnect as long as the Pool is not dropped.
        // As the messages are processed using another task, we have no loop on the main "thread" to check for connection lost. When the messaging task
        // detects a connection lost, it will notify the reconnect task to reconnect. As long as the Pool is not dropped, the reconnect task will keep
        // trying to reconnect for each notification.
        // In a similar way when a handler is added the reconnect task will be triggered to reconnect.
        {
            let inner = inner.clone();
            let notify = notify.clone();
            tokio::spawn(async move {
                notify.0.notified().await;
                log::info!("PGListener reconnecting...");

                if !notify.1.load(Ordering::Relaxed) {
                    let mut inner = inner.lock().await;
                    let inner = &mut *inner;
                    if let Err(err) = inner.connect().await {
                        log::error!("PGListener failed to reconnect: {:#?}", err);
                    }
                } else {
                    log::info!("PGListener is closed");
                }
            });
        }

        Self { notify, inner }
    }

    pub fn close(&self) {
        self.notify.1.store(true, Ordering::Relaxed);
    }

    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), DBError>
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let mut inner = self.inner.lock().await;
        let inner = &mut *inner;

        // make sure the listener is connected
        if !inner.is_connected() {
            log::info!("PGListener triggering a reconnection for a new handler...");
            self.notify.0.notify_one();
        }

        inner.listen(channel, handler).await?;

        Ok(())
    }

    pub async fn unlisten(&self, channel: &str) -> Result<(), DBError> {
        let mut inner = self.inner.lock().await;
        let inner = &mut *inner;
        inner.unlisten(channel).await?;

        Ok(())
    }

    pub async fn unlisten_all(&mut self) -> Result<(), DBError> {
        let mut inner = self.inner.lock().await;
        let inner = &mut *inner;
        inner.unlisten_all().await?;

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
