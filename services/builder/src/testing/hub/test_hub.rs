use crate::{
    models::{
        messages::{DisconnectReason, HubEvent, HubMessage, PingEvent, SubscribeFailure, TopicKey},
        HubError,
    },
    repositories::{
        command_inbox::redis::RedisCommandInboxDb,
        command_log::pg::PgCommandLogDb,
        hub_registry::{redis::RedisHubConnectionDb, HubConnectionDb, HubRegistry},
    },
    services::{standard_registry, HubConfig, HubService, StreamAuthorizeService, StreamConfig, StreamRegistry},
    testing::command_stream::{TestInbox, TestLog},
};
use ring::rand::SystemRandom;
use shine_game::replica::StreamId;
use shine_infra::{
    db::redis::RedisConnectionPool,
    session::{CurrentUserService, SessionKey},
};
use shine_infra_testing::db::redis::TestRedis;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
    vec,
};
use tokio::{
    sync::mpsc::{self, error::TryRecvError},
    time::timeout,
};
use uuid::Uuid;

const BARRIER_TIMEOUT: Duration = Duration::from_secs(30);
const IDLE: Duration = Duration::from_secs(3600);

/// A connection's lifecycle events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Connected(Uuid),
    Disconnected(Uuid),
}

impl Lifecycle {
    /// Collects the lifecycle events of all users, grouped by user ID.
    pub fn collect(events: &[HubEvent]) -> HashMap<Uuid, Vec<Lifecycle>> {
        let mut by_user = HashMap::<Uuid, Vec<Lifecycle>>::new();
        for event in events {
            let (user_id, lifecycle) = match event {
                HubEvent::UserConnected { user_id, connection_id, .. } => {
                    (*user_id, Lifecycle::Connected(*connection_id))
                }
                HubEvent::UserDisconnected { user_id, connection_id, .. } => {
                    (*user_id, Lifecycle::Disconnected(*connection_id))
                }
                HubEvent::Shutdown => panic!("unexpected Shutdown event"),
            };
            by_user.entry(user_id).or_default().push(lifecycle);
        }
        by_user
    }

    /// Collects the lifecycle events of a specific user.
    pub fn collect_by_user(events: &[HubEvent], user_id: Uuid) -> Vec<Lifecycle> {
        Self::collect(events).remove(&user_id).unwrap_or_default()
    }

    /// Collects the lifecycle events of a specific user.
    pub fn assert_by_user(events: &[HubEvent], user_id: Uuid, expected: &[Lifecycle]) -> Vec<Lifecycle> {
        let actual = Self::collect(events).remove(&user_id).unwrap_or_default();
        assert_eq!(&actual, expected);
        actual
    }

    /// Connections removed by the hub, by reason.
    pub fn collect_drops(events: &[HubEvent]) -> BTreeMap<DisconnectReason, u64> {
        let mut drops = BTreeMap::new();
        for event in events {
            if let HubEvent::UserDisconnected { reason, .. } = event {
                if reason.is_drop() {
                    *drops.entry(*reason).or_default() += 1;
                }
            }
        }
        drops
    }

    pub fn assert_drops(events: &[HubEvent], expected: &[(DisconnectReason, u64)]) {
        let actual = Self::collect_drops(events);
        let expected_map: BTreeMap<DisconnectReason, u64> = expected.iter().cloned().collect();
        assert_eq!(actual, expected_map);
    }
}

pub type Egress = mpsc::Receiver<HubMessage>;

/// Collect egress state from the hub messages.
#[derive(Debug, Default)]
pub struct EgressState {
    pub pings: Vec<String>,
    pub disconnects: Vec<DisconnectReason>,
    pub closed: bool,
}

impl EgressState {
    pub async fn poll(egress: &mut Egress) -> Option<HubMessage> {
        tokio::time::timeout(BARRIER_TIMEOUT, egress.recv()).await.unwrap()
    }

    pub fn from_egress(egress: &mut Egress) -> Self {
        let mut state = EgressState::default();
        state.receive(egress);
        state
    }

    /// Records a message and filter by connection ID if provided.
    pub fn record(&mut self, message: HubMessage, connect_id: Option<Uuid>) {
        match message {
            HubMessage::Ping(event) => self.pings.push(event.id),
            HubMessage::Hub(HubEvent::UserDisconnected { connection_id, reason, .. })
                if reason.is_drop() && connect_id.is_none_or(|own| own == connection_id) =>
            {
                self.disconnects.push(reason)
            }
            HubMessage::Hub(_) | HubMessage::StreamMessage(_) | HubMessage::StreamEvent(_) => {}
        }
    }

    pub fn receive(&mut self, egress: &mut Egress) {
        loop {
            match egress.try_recv() {
                Ok(message) => self.record(message, None),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    return;
                }
            }
        }
    }

    pub fn assert_open(&self) -> &Self {
        assert!(!self.closed, "expected egress to be open");
        self
    }

    pub fn assert_closed(&self) -> &Self {
        assert!(self.closed, "expected egress to be closed");
        self
    }

    pub fn assert_pings(&self, expected: &[&str]) -> &Self {
        assert_eq!(self.pings, expected);
        self
    }

    pub fn assert_disconnects(&self, expected: &[DisconnectReason]) -> &Self {
        assert_eq!(self.disconnects, expected);
        self
    }

    pub fn assert_last_disconnect(&self, reason: DisconnectReason) -> &Self {
        assert!(
            self.disconnects.ends_with(&[reason]),
            "expected egress to be closed with reason: {:?}",
            reason
        );
        self
    }
}

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

    pub redis: RedisConnectionPool,
    pub registry: RedisHubConnectionDb,
    pub direct_redis: RedisConnectionPool,
    pub direct_registry: RedisHubConnectionDb,
    pub events: mpsc::UnboundedReceiver<HubMessage>,
}

impl TestHub {
    pub async fn default_from_env() -> Option<Self> {
        Self::from_env(TestHubOptions::default()).await
    }

    pub async fn from_env(options: TestHubOptions) -> Option<Self> {
        let TestRedis {
            pool: redis,
            direct_pool: direct_redis,
        } = TestRedis::from_env().await?;

        let hub_config = HubConfig {
            heartbeat: options.heartbeat,
            session_check: options.session_check,
            stream_shutdown_timeout: Duration::from_secs(10),
            ping_enabled: options.ping_enabled,
        };

        // URL_SAFE_NO_PAD: 86 'A' chars decode to 64 zero bytes, a valid cookie key for tests.
        let cookie_secret = "A".repeat(86);
        let session_service = Arc::new(CurrentUserService::new(None, &cookie_secret, "", 120, redis.clone()).unwrap());

        let registry = RedisHubConnectionDb::new(&redis, 120).await.unwrap();
        let direct_registry = RedisHubConnectionDb::new(&direct_redis, 120).await.unwrap();
        let mut stream_inbox = None;
        let mut stream_log = None;
        let mut stream_registry = None;

        let mut hub_builder = HubService::builder(hub_config)
            .with_session_service(session_service)
            .with_hub_registry(registry.clone());
        if let Some(config) = options.with_streams {
            let si = TestInbox::from_db(redis.clone()).await.unwrap();
            let sl = TestLog::from_env().await.unwrap();
            hub_builder = hub_builder
                .with_stream_authorize_service(StreamAuthorizeService::new)
                .with_stream_registry(|egress| {
                    let sr = standard_registry(si.db.clone(), sl.db.clone(), egress, config)?;
                    stream_inbox = Some(si);
                    stream_log = Some(sl);
                    stream_registry = Some(sr.clone());
                    Ok(sr)
                })
                .unwrap()
        }
        let hub = hub_builder.build().await.unwrap();

        let events = hub.subscribe(vec![TopicKey::Hub]).await;
        Some(TestHub {
            hub,
            stream_inbox,
            stream_log,
            stream_registry,
            redis,
            registry,
            direct_redis,
            direct_registry,
            events,
        })
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

    pub fn connect(
        &self,
        user_id: Uuid,
        capacity: usize,
        topics: Vec<TopicKey>,
        session_key_override: Option<SessionKey>,
    ) -> (Uuid, Egress) {
        let (tx, rx) = mpsc::channel(capacity);
        let connection_id = self
            .hub
            .request_connection(user_id, session_key_override.unwrap_or_else(session_key), tx, topics)
            .unwrap();
        (connection_id, rx)
    }

    pub fn connect_ping(&self, user_id: Uuid, capacity: usize) -> (Uuid, Egress) {
        self.connect(user_id, capacity, vec![TopicKey::Ping], None)
    }

    //todo: should be part of another test tool
    pub async fn seed_session(&self, user_id: Uuid) -> SessionKey {
        let session_key = session_key();
        let prefix = format!("session:{}:{}", user_id.as_simple(), session_key.key_hash());
        let sentinel = serde_json::json!({ "createdAt": "2026-01-01T00:00:00Z", "fingerprint": "" });
        let data = serde_json::json!({ "name": "test", "isEmailConfirmed": false, "isLinked": false, "roles": [] });
        let mut conn = self.direct_redis.get().await.unwrap();
        let _: () = redis::pipe()
            .set_ex(format!("{prefix}:sentinel"), sentinel.to_string(), 120)
            .set_ex(format!("{prefix}:data"), data.to_string(), 120)
            .query_async(&mut *conn)
            .await
            .unwrap();
        session_key
    }

    pub async fn send_ping_to_connection(&self, connection: Uuid, id: impl Into<String>) -> Result<bool, HubError> {
        self.hub
            .send_to_connection(connection, HubMessage::Ping(PingEvent { id: id.into() }))
            .await
    }

    pub async fn send_ping_to_user(&self, user: Uuid, id: impl Into<String>) -> Result<(), HubError> {
        self.hub
            .send_to_user(user, HubMessage::Ping(PingEvent { id: id.into() }))
            .await
    }

    pub async fn send_ping_to_all(&self, id: impl Into<String>) -> Result<(), HubError> {
        self.hub.broadcast(HubMessage::Ping(PingEvent { id: id.into() })).await
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
    ) -> Option<Vec<HubEvent>> {
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
        .then_some(collected)
    }

    async fn wait_for_barrier(hub: &HubService, events: &mut mpsc::UnboundedReceiver<HubMessage>) -> Vec<HubEvent> {
        // create a sentinel connection to act as a barrier for all previous commands
        let (sentinel_user, _sentinel_rx) = {
            let sentinel_user = Uuid::new_v4();
            let (tx, rx) = mpsc::channel(1);
            let connection_id = hub
                .request_connection(sentinel_user, session_key(), tx, Vec::new())
                .unwrap();
            hub.request_disconnection(sentinel_user, connection_id).unwrap();
            (sentinel_user, rx)
        };

        let mut collected = vec![];
        loop {
            match timeout(BARRIER_TIMEOUT, events.recv()).await {
                Ok(Some(HubMessage::Hub(event))) => match event {
                    HubEvent::UserDisconnected { user_id, .. } if user_id == sentinel_user => return collected,
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
    pub async fn wait_all_commands(&mut self) -> Vec<HubEvent> {
        Self::wait_for_barrier(&self.hub, &mut self.events).await
    }

    /// Waits for queued commands using a specific subscriber.
    pub async fn wait_all_commands_on(&self, events: &mut mpsc::UnboundedReceiver<HubMessage>) -> Vec<HubEvent> {
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
        let mut conn = self.direct_redis.get().await.unwrap();
        let key = registry_key(user_id);
        let _: () = redis::cmd("DEL").arg(key).query_async(&mut *conn).await.unwrap();
    }

    pub async fn set_connection_ttl(&self, user_id: Uuid, seconds: u64) {
        let mut conn = self.direct_redis.get().await.unwrap();
        let _: () = redis::cmd("EXPIRE")
            .arg(registry_key(user_id))
            .arg(seconds)
            .query_async(&mut *conn)
            .await
            .unwrap();
    }

    pub async fn get_connection_ttl(&self, user_id: Uuid) -> i64 {
        let mut conn = self.direct_redis.get().await.unwrap();
        redis::cmd("TTL")
            .arg(registry_key(user_id))
            .query_async(&mut *conn)
            .await
            .unwrap()
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

fn registry_key(user_id: Uuid) -> String {
    format!("hub-connection:{}", user_id.as_simple())
}

fn session_key() -> SessionKey {
    SessionKey::new_random(&SystemRandom::new()).unwrap()
}
