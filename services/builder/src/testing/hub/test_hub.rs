use crate::{
    models::{
        messages::{HubEvent, HubMessage, SubscribeFailure, TopicKey},
        HubError,
    },
    repositories::{
        command_inbox::redis::RedisCommandInboxDb,
        command_log::pg::PgCommandLogDb,
        hub_registry::{
            redis::{hub_connection_key, RedisHubConnectionDb},
            HubConnectionDb, HubRegistry,
        },
    },
    services::{
        standard_registry, HubConfig, HubService, StreamAuthorizeService, StreamConfig, StreamRegistry, StreamsConfig,
    },
    testing::{
        command_stream::{TestInbox, TestLog},
        hub::{Egress, HubEvents, TestSession},
    },
};
use shine_game::replica::StreamId;
use shine_infra::session::{CurrentUserService, SessionKey};
use shine_infra_testing::db::redis::TestRedis;
use std::{collections::HashMap, sync::Arc, time::Duration, vec};
use tokio::{sync::mpsc, time::timeout};
use uuid::Uuid;

pub(super) const BARRIER_TIMEOUT: Duration = Duration::from_secs(30);
const IDLE: Duration = Duration::from_secs(3600);

/// Hub configuration of a test. The background consumers idle unless a test shortens their interval, and
/// every hub chats in its own room so no other test's appends wake it.
pub struct TestHubOptions {
    pub heartbeat: Duration,
    pub session_check: Duration,
    pub ping_enabled: bool,
    pub with_streams: Option<StreamConfig>,
}

impl Default for TestHubOptions {
    fn default() -> Self {
        Self {
            heartbeat: IDLE,
            session_check: IDLE,
            ping_enabled: true,
            with_streams: None,
        }
    }
}

pub struct TestHub {
    pub hub: HubService,

    pub stream_inbox: Option<TestInbox>,
    pub stream_log: Option<TestLog>,
    pub stream_registry: Option<StreamRegistry<RedisCommandInboxDb, PgCommandLogDb>>,

    pub redis: TestRedis,
    pub registry: RedisHubConnectionDb,
    pub direct_registry: RedisHubConnectionDb,
    pub events: mpsc::UnboundedReceiver<HubMessage>,
}

impl TestHub {
    pub async fn default_from_env() -> Self {
        Self::from_env(TestHubOptions::default()).await
    }

    pub async fn from_env(options: TestHubOptions) -> Self {
        let redis = TestRedis::from_env().await;

        let hub_config = HubConfig {
            heartbeat: options.heartbeat,
            session_check: options.session_check,
            stream_shutdown_timeout: Duration::from_secs(10),
            ping_enabled: options.ping_enabled,
        };

        // URL_SAFE_NO_PAD: 86 'A' chars decode to 64 zero bytes, a valid cookie key for tests.
        let cookie_secret = "A".repeat(86);
        let session_service =
            Arc::new(CurrentUserService::new(None, &cookie_secret, "", 120, redis.pool.clone()).unwrap());

        let registry = RedisHubConnectionDb::new(&redis.pool, 120).await.unwrap();
        let direct_registry = RedisHubConnectionDb::new(&redis.direct_pool, 120).await.unwrap();
        let mut stream_inbox = None;
        let mut stream_log = None;
        let mut stream_registry = None;

        let mut hub_builder = HubService::builder(hub_config)
            .with_session_service(session_service)
            .with_hub_registry(registry.clone())
            .with_stream_authorize_service(StreamAuthorizeService::new);
        if let Some(config) = options.with_streams {
            let si = TestInbox::new(redis.clone(), 60).await;
            let sl = TestLog::from_env().await;
            hub_builder = hub_builder
                .with_stream_registry(|egress| {
                    let sr = standard_registry(si.db.clone(), sl.db.clone(), egress, StreamsConfig::uniform(config))?;
                    stream_inbox = Some(si);
                    stream_log = Some(sl);
                    stream_registry = Some(sr.clone());
                    Ok(sr)
                })
                .unwrap()
        }
        let hub = hub_builder.build().await.unwrap();

        let events = hub.subscribe(vec![TopicKey::Hub]).await;
        TestHub {
            hub,
            stream_inbox,
            stream_log,
            stream_registry,
            redis,
            registry,
            direct_registry,
            events,
        }
    }

    /// Warms up the connection pool by performing a few registry lookups.
    pub async fn warmup(&self) {
        let lookups: Vec<_> = (0..4)
            .map(|_| {
                let registry = self.registry.clone();
                tokio::spawn(async move { Self::find_registry_connection_by_user(&registry, Uuid::new_v4()).await })
            })
            .collect();
        for lookup in lookups {
            lookup.await.unwrap();
        }
    }

    pub const BARRIER_TIMEOUT: Duration = BARRIER_TIMEOUT;

    pub fn connect(
        &self,
        user_id: Uuid,
        capacity: usize,
        topics: Vec<TopicKey>,
        session_key: Option<SessionKey>,
    ) -> (Uuid, Egress) {
        Self::connect_to(&self.hub, user_id, capacity, topics, session_key)
    }

    /// Connects to a hub that is not (or no longer) owned by a `TestHub`, e.g. from a spawned task.
    pub fn connect_to(
        hub: &HubService,
        user_id: Uuid,
        capacity: usize,
        topics: Vec<TopicKey>,
        session_key: Option<SessionKey>,
    ) -> (Uuid, Egress) {
        let (tx, rx) = mpsc::channel(capacity);
        let connection_id = hub
            .request_connection(
                user_id,
                session_key.unwrap_or_else(TestSession::new_random_key),
                tx,
                topics,
            )
            .unwrap();
        (connection_id, Egress::new(rx))
    }

    pub fn connect_ping(&self, user_id: Uuid, capacity: usize) -> (Uuid, Egress) {
        self.connect(user_id, capacity, vec![TopicKey::Ping], None)
    }

    pub fn connect_ping_to(hub: &HubService, user_id: Uuid, capacity: usize) -> (Uuid, Egress) {
        Self::connect_to(hub, user_id, capacity, vec![TopicKey::Ping], None)
    }

    pub async fn send_to_connection(&self, connection: Uuid, message: HubMessage) -> Result<bool, HubError> {
        self.hub.send_to_connection(connection, message).await
    }

    pub async fn send_to_user(&self, user: Uuid, message: HubMessage) -> Result<(), HubError> {
        self.hub.send_to_user(user, message).await
    }

    pub async fn broadcast(&self, message: HubMessage) -> Result<(), HubError> {
        self.hub.broadcast(message).await
    }

    pub async fn stream_subscribe(
        &self,
        connection_id: Uuid,
        user_id: Uuid,
        stream_id: &StreamId,
    ) -> Result<(), SubscribeFailure> {
        self.hub
            .subscribe_stream(connection_id, user_id, stream_id.clone())
            .await
    }

    /// Waits for the next event matching `predicate`, collecting the ones before it.
    pub async fn wait_event(
        &mut self,
        window: Option<Duration>,
        predicate: impl Fn(&HubEvent) -> bool,
    ) -> Option<HubEvents> {
        let mut collected = vec![];

        timeout(window.unwrap_or(BARRIER_TIMEOUT), async {
            while let Some(message) = self.events.recv().await {
                if let HubMessage::Hub(event) = message {
                    let found = predicate(&event);
                    collected.push(event);
                    if found {
                        return true;
                    }
                }
            }
            false
        })
        .await
        .unwrap_or(false)
        .then(|| collected.into())
    }

    async fn wait_for_barrier(hub: &HubService, events: &mut mpsc::UnboundedReceiver<HubMessage>) -> HubEvents {
        // create a sentinel connection to act as a barrier for all previous commands
        let (sentinel_user, _sentinel_rx) = {
            let sentinel_user = Uuid::new_v4();
            let (tx, rx) = mpsc::channel(1);
            let connection_id = hub
                .request_connection(sentinel_user, TestSession::new_random_key(), tx, Vec::new())
                .unwrap();
            hub.request_disconnection(sentinel_user, connection_id).unwrap();
            (sentinel_user, rx)
        };

        let mut collected = vec![];
        loop {
            match timeout(BARRIER_TIMEOUT, events.recv()).await {
                Ok(Some(HubMessage::Hub(event))) => match event {
                    HubEvent::UserDisconnected { user_id, .. } if user_id == sentinel_user => return collected.into(),
                    HubEvent::UserConnected { user_id, .. } if user_id == sentinel_user => {}
                    event => collected.push(event),
                },
                Ok(Some(_)) => {}
                Ok(None) => panic!("hub event stream closed before the barrier"),
                Err(_) => panic!("timed out waiting for the hub barrier"),
            }
        }
    }

    /// Waits until all commands enqueued so far are processed, and returns the events published meanwhile.
    pub async fn wait_all_commands(&mut self) -> HubEvents {
        Self::wait_for_barrier(&self.hub, &mut self.events).await
    }

    /// Waits for queued commands using a specific subscriber.
    pub async fn wait_all_commands_on(&self, events: &mut mpsc::UnboundedReceiver<HubMessage>) -> HubEvents {
        Self::wait_for_barrier(&self.hub, events).await
    }

    /// Waits until the control channel is closed, indicating that the hub is shutting down.
    pub async fn wait_for_shutdown_events(&mut self) -> Vec<HubMessage> {
        let mut messages = vec![];
        while let Some(message) = timeout(BARRIER_TIMEOUT, self.events.recv())
            .await
            .expect("shutdown timed out")
        {
            messages.push(message);
        }
        messages
    }

    /// Waits until the hub has fully shut down, or times out.
    pub async fn wait_shutdown_complete(&self) {
        timeout(BARRIER_TIMEOUT, self.hub.wait_shutdown_complete())
            .await
            .expect("hub shutdown timed out");
    }

    async fn find_registry_connection_by_user(registry: &RedisHubConnectionDb, user_id: Uuid) -> Option<Uuid> {
        let mut context = registry.create_context().await.unwrap();
        context
            .find_connection_by_user(user_id)
            .await
            .unwrap()
            .map(|connection| connection.connection_id)
    }

    /// Finds the active connection for the specified user in the registry. Returns `None` if no connection exists.
    pub async fn find_connection_by_user(&self, user_id: Uuid) -> Option<Uuid> {
        Self::find_registry_connection_by_user(&self.direct_registry, user_id).await
    }

    /// Deletes the connection for the specified user from registry.
    pub async fn delete_connection_by_user(&self, user_id: Uuid) {
        self.redis.del(&hub_connection_key(user_id)).await;
    }

    pub async fn set_connection_ttl(&self, user_id: Uuid, seconds: u64) {
        self.redis.expire(&hub_connection_key(user_id), seconds).await;
    }

    pub async fn get_connection_ttl(&self, user_id: Uuid) -> i64 {
        self.redis.ttl(&hub_connection_key(user_id)).await
    }

    /// The hub's local view, its registry and the events all agree on the active connection of each user.
    pub async fn assert_consistent(&self, users: impl IntoIterator<Item = Uuid>, active: &HashMap<Uuid, Uuid>) {
        assert_eq!(
            self.hub.stats().await.connections,
            active.len(),
            "local connection count"
        );
        for user_id in users {
            assert_eq!(
                self.find_connection_by_user(user_id).await,
                active.get(&user_id).copied(),
                "registry entry of {user_id}"
            );
        }
    }

    /// Disconnects the specified connection and waits until all commands are processed.
    pub async fn cleanup_connection(&mut self, user: Uuid, second: Uuid) {
        self.hub.request_disconnection(user, second).unwrap();
        self.wait_all_commands().await;
    }
}
