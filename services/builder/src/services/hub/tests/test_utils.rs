use crate::models::messages::PingEvent;
use crate::repositories::{command_inbox::redis::RedisCommandInboxDb, command_log::pg::PgCommandLogDb};
use crate::services::command_stream::{standard_registry, CommandStreamConfig, WorkerConfig};
use crate::services::{HubConfig, HubService, StreamAuthorizeService};
use crate::{
    models::messages::{DisconnectReason, HubEvent, HubMessage, TopicKey},
    repositories::hub_registry::{redis::RedisHubConnectionDb, HubConnectionDb, HubRegistry},
};
use ring::rand::SystemRandom;
use shine_infra::{
    db::redis::{create_redis_pool, RedisConnectionPool},
    session::{CurrentUserService, SessionKey},
};
use shine_infra_testing::db::redis::get_redis_cns;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::mpsc::{self, error::TryRecvError},
    time::timeout,
};
use uuid::Uuid;

pub const BARRIER_TIMEOUT: Duration = Duration::from_secs(30);
const IDLE: Duration = Duration::from_secs(3600);

pub type Events = mpsc::UnboundedReceiver<HubMessage>;
pub type Egress = mpsc::Receiver<HubMessage>;

/// Hub configuration of a test. The background consumers idle unless a test shortens their interval, and
/// every hub chats in its own room so no other test's appends wake it.
pub struct TestHubOptions {
    pub heartbeat: Duration,
    pub session_check: Duration,
    pub ping_enabled: bool,
    pub redis_cns: Option<String>,
    pub streams: Option<(PgCommandLogDb, RedisCommandInboxDb, WorkerConfig)>,
}

impl Default for TestHubOptions {
    fn default() -> Self {
        Self {
            heartbeat: IDLE,
            session_check: IDLE,
            ping_enabled: true,
            redis_cns: None,
            streams: None,
        }
    }
}

pub struct TestHub {
    pub hub: HubService,
    pub registry: RedisHubConnectionDb,
    pub redis: RedisConnectionPool,
    pub events: Events,
}

/// A hub on its own Redis pool, i.e. with its own registry listener: two of them act as two service
/// instances. `None` when no test Redis is configured.
pub async fn create_test_hub(options: TestHubOptions) -> Option<TestHub> {
    let redis_cns = options.redis_cns.or(get_redis_cns())?;
    let redis = create_redis_pool(redis_cns.as_str()).await.unwrap();
    let registry = RedisHubConnectionDb::new(&redis, 120).await.unwrap();

    // URL_SAFE_NO_PAD: 86 'A' chars decode to 64 zero bytes, a valid cookie key for tests.
    let cookie_secret = "A".repeat(86);
    let session_service = Arc::new(CurrentUserService::new(None, &cookie_secret, "", 120, redis.clone()).unwrap());

    let hub = {
        let hub_config = HubConfig {
            heartbeat: options.heartbeat,
            session_check: options.session_check,
            stream_shutdown_timeout: Duration::from_secs(10),
            ping_enabled: options.ping_enabled,
        };

        let mut hub_builder = HubService::builder(hub_config)
            .with_session_service(session_service)
            .with_hub_registry(registry.clone())
            .with_stream_authorize_service(StreamAuthorizeService::new);
        if let Some((log_db, inbox_db, worker)) = options.streams {
            hub_builder = hub_builder
                .with_stream_registry(|egress| {
                    standard_registry(inbox_db, log_db, egress, CommandStreamConfig::default(), worker)
                })
                .unwrap()
        };
        hub_builder.build().await.unwrap()
    };

    let events = hub.subscribe(vec![TopicKey::Hub]).await;
    Some(TestHub { hub, registry, redis, events })
}

pub fn key() -> SessionKey {
    SessionKey::new_random(&SystemRandom::new()).unwrap()
}

pub fn ping(id: impl Into<String>) -> HubMessage {
    HubMessage::Ping(PingEvent { id: id.into() })
}

/// Requests a connection on the Chat topic, as the ws sockets do.
pub fn connect(hub: &HubService, user_id: Uuid, capacity: usize) -> (Uuid, Egress) {
    connect_on(hub, user_id, capacity, vec![TopicKey::Ping])
}

pub fn connect_on(hub: &HubService, user_id: Uuid, capacity: usize, topics: Vec<TopicKey>) -> (Uuid, Egress) {
    let (tx, rx) = mpsc::channel(capacity);
    let connection_id = hub.request_connection(user_id, key(), tx, topics).unwrap();
    (connection_id, rx)
}

/// Waits until every command enqueued so far is processed, and returns the events published meanwhile.
pub async fn barrier(hub: &HubService, events: &mut Events) -> Vec<HubEvent> {
    let sentinel_user = Uuid::new_v4();
    let (sentinel, _sentinel_rx) = connect_on(hub, sentinel_user, 1, vec![]);
    hub.request_disconnection(sentinel_user, sentinel).unwrap();

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

/// Waits for the next event matching `predicate`, collecting the ones before it.
pub async fn wait_event(
    events: &mut Events,
    window: Duration,
    predicate: impl Fn(&HubEvent) -> bool,
) -> Option<Vec<HubEvent>> {
    let mut collected = vec![];
    timeout(window, async {
        while let Some(message) = events.recv().await {
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

/// Waits until the command loop has exited, which drops the control channel.
pub async fn wait_stopped(hub: &HubService) {
    timeout(BARRIER_TIMEOUT, async {
        while hub.notify_registry_changed(Uuid::nil()).is_ok() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the hub command loop did not stop");
}

/// A connection's lifecycle event, without the session key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Connected(Uuid),
    Disconnected(Uuid),
}

pub fn lifecycle_by_user(events: &[HubEvent]) -> HashMap<Uuid, Vec<Lifecycle>> {
    let mut by_user = HashMap::<Uuid, Vec<Lifecycle>>::new();
    for event in events {
        let (user_id, lifecycle) = match event {
            HubEvent::UserConnected { user_id, connection_id, .. } => (*user_id, Lifecycle::Connected(*connection_id)),
            HubEvent::UserDisconnected { user_id, connection_id, .. } => {
                (*user_id, Lifecycle::Disconnected(*connection_id))
            }
            HubEvent::Shutdown => panic!("unexpected Shutdown event"),
        };
        by_user.entry(user_id).or_default().push(lifecycle);
    }
    by_user
}

pub fn lifecycle_of(events: &[HubEvent], user_id: Uuid) -> Vec<Lifecycle> {
    lifecycle_by_user(events).remove(&user_id).unwrap_or_default()
}

pub fn count_connects(events: &[HubEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, HubEvent::UserConnected { .. }))
        .count()
}

/// Replays the events, asserting the lifecycle invariants: a user has at most one active connection, and
/// a connection connects once and disconnects at most once, only while it is the active one. Returns the
/// active connection per user.
pub fn replay(events: &[HubEvent]) -> HashMap<Uuid, Uuid> {
    let mut active = HashMap::new();
    let mut connected = HashSet::new();
    let mut disconnected = HashSet::new();
    for event in events {
        match event {
            HubEvent::UserConnected { user_id, connection_id, .. } => {
                assert!(connected.insert(*connection_id), "{connection_id} connected twice");
                let previous = active.insert(*user_id, *connection_id);
                assert!(
                    previous.is_none(),
                    "{user_id} got {connection_id} while {previous:?} was still active"
                );
            }
            HubEvent::UserDisconnected { user_id, connection_id, .. } => {
                assert!(
                    disconnected.insert(*connection_id),
                    "{connection_id} disconnected twice"
                );
                assert_eq!(
                    active.remove(user_id),
                    Some(*connection_id),
                    "{user_id} disconnected {connection_id}, which is not its active connection"
                );
            }
            HubEvent::Shutdown => panic!("unexpected Shutdown event"),
        }
    }
    active
}

/// What a connection's egress channel holds once the hub is idle.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct EgressState {
    pub pings: Vec<String>,
    pub notices: Vec<DisconnectReason>,
    pub closed: bool,
}

impl EgressState {
    pub fn record(&mut self, message: HubMessage) {
        self.record_for(None, message);
    }

    /// Records a message; a connection on the Hub topic passes its id to skip the drops of others.
    pub fn record_for(&mut self, own: Option<Uuid>, message: HubMessage) {
        match message {
            HubMessage::Ping(event) => self.pings.push(event.id),
            HubMessage::Hub(HubEvent::UserDisconnected { connection_id, reason, .. })
                if reason.is_drop() && own.is_none_or(|own| own == connection_id) =>
            {
                self.notices.push(reason)
            }
            HubMessage::Hub(_) | HubMessage::StreamMessage(_) | HubMessage::StreamEvent(_) => {}
        }
    }
}

pub fn egress_state(rx: &mut Egress) -> EgressState {
    let mut state = EgressState::default();
    loop {
        match rx.try_recv() {
            Ok(message) => state.record(message),
            Err(TryRecvError::Empty) => return state,
            Err(TryRecvError::Disconnected) => {
                state.closed = true;
                return state;
            }
        }
    }
}

/// Collects a connection's egress until the hub releases the channel.
pub fn spawn_reader(connection_id: Uuid, mut rx: Egress) -> tokio::task::JoinHandle<EgressState> {
    tokio::spawn(async move {
        let mut state = EgressState::default();
        while let Some(message) = rx.recv().await {
            state.record_for(Some(connection_id), message);
        }
        state.closed = true;
        state
    })
}

pub fn open() -> EgressState {
    EgressState::default()
}

pub fn closed_with(notice: Option<DisconnectReason>) -> EgressState {
    EgressState {
        pings: vec![],
        notices: notice.into_iter().collect(),
        closed: true,
    }
}

/// Connections removed by the hub, by reason, as announced by the `UserDisconnected` events.
pub fn drops(events: &[HubEvent]) -> BTreeMap<DisconnectReason, u64> {
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

pub fn dropped(entries: &[(DisconnectReason, u64)]) -> BTreeMap<DisconnectReason, u64> {
    entries.iter().copied().collect()
}

pub async fn find_registry_connection(registry: &RedisHubConnectionDb, user_id: Uuid) -> Option<Uuid> {
    let mut context = registry.create_context().await.unwrap();
    context
        .find_connection_by_user(user_id)
        .await
        .unwrap()
        .map(|connection| connection.connection_id)
}

/// The hub's local view, its registry and the events all agree on the active connection of each user.
pub async fn assert_consistent(
    hub: &HubService,
    registry: &RedisHubConnectionDb,
    users: impl IntoIterator<Item = Uuid>,
    active: &HashMap<Uuid, Uuid>,
) {
    assert_eq!(hub.stats().await.connections, active.len(), "local connection count");
    for user_id in users {
        assert_eq!(
            find_registry_connection(registry, user_id).await,
            active.get(&user_id).copied(),
            "registry entry of {user_id}"
        );
    }
}

pub fn registry_key(user_id: Uuid) -> String {
    format!("hub-connection:{}", user_id.as_simple())
}

/// A registry on the test Redis, bypassing any proxy of the hub under test.
pub async fn direct_registry() -> RedisHubConnectionDb {
    let cns = get_redis_cns().unwrap();
    let redis = create_redis_pool(&cns).await.unwrap();
    RedisHubConnectionDb::new(&redis, 120).await.unwrap()
}

/// Stores a valid session, as the identity service does at login.
pub async fn seed_session(redis: &RedisConnectionPool, user_id: Uuid, session_key: SessionKey) {
    let prefix = format!("session:{}:{}", user_id.as_simple(), session_key.key_hash());
    let sentinel = serde_json::json!({ "createdAt": "2026-01-01T00:00:00Z", "fingerprint": "" });
    let data = serde_json::json!({ "name": "test", "isEmailConfirmed": false, "isLinked": false, "roles": [] });
    let mut conn = redis.get().await.unwrap();
    let _: () = redis::pipe()
        .set_ex(format!("{prefix}:sentinel"), sentinel.to_string(), 120)
        .set_ex(format!("{prefix}:data"), data.to_string(), 120)
        .query_async(&mut *conn)
        .await
        .unwrap();
}

pub async fn delete_key(redis: &RedisConnectionPool, key: &str) {
    let mut conn = redis.get().await.unwrap();
    let _: () = redis::cmd("DEL").arg(key).query_async(&mut *conn).await.unwrap();
}

/// Requests a connection with a given session key, on the Chat topic.
pub fn connect_with_key(hub: &HubService, user_id: Uuid, session_key: SessionKey, capacity: usize) -> (Uuid, Egress) {
    let (tx, rx) = mpsc::channel(capacity);
    let connection_id = hub
        .request_connection(user_id, session_key, tx, vec![TopicKey::Ping])
        .unwrap();
    (connection_id, rx)
}

/// Small deterministic generator (SplitMix64), so a failing script replays with the same seed.
pub struct Rng(pub u64);

impl Rng {
    pub fn below(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) % n
    }
}
