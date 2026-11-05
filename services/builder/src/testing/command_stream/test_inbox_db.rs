use crate::repositories::command_inbox::{redis::RedisCommandInboxDb, CommandInboxDb, InboxId, InboxWake};
use shine_game::replica::StreamId;
use shine_infra_testing::db::redis::TestRedis;
use std::time::Duration;
use tokio::{
    sync::mpsc,
    time::{sleep, timeout},
};

const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct TestInbox {
    pub redis: TestRedis,
    pub db: RedisCommandInboxDb,
}

impl TestInbox {
    pub async fn new(redis: TestRedis, ttl_seconds: u64) -> TestInbox {
        let db = RedisCommandInboxDb::new(&redis.pool, ttl_seconds).await.unwrap();
        TestInbox { redis, db }
    }

    pub async fn from_env_with_ttl(ttl_seconds: u64) -> TestInbox {
        let redis = TestRedis::from_env().await;
        Self::new(redis, ttl_seconds).await
    }

    /// An inbox on the test Redis. Tests share the Redis and stay apart by using a stream of their own.
    pub async fn from_env() -> TestInbox {
        Self::from_env_with_ttl(60).await
    }

    /// Adds a raw entry, to put in the inbox what the repository itself never would.
    pub async fn raw_add(&self, stream_id: &StreamId, fields: &[(&str, &str)]) -> InboxId {
        let id: String = self.xadd(stream_id, "*", fields).await;
        InboxId::parse(&id).unwrap()
    }

    /// Adds a raw entry at a given position, in an inbox that has none above it (see `drop_inbox`).
    pub async fn raw_add_at(&self, stream_id: &StreamId, at: &InboxId, fields: &[(&str, &str)]) {
        let _: String = self.xadd(stream_id, &at.to_string(), fields).await;
    }

    /// Deletes the whole inbox, with the memory of its last id, as if it had expired.
    pub async fn drop_inbox(&self, stream_id: &StreamId) {
        self.redis.del(&Self::key(stream_id)).await;
    }

    pub async fn len(&self, stream_id: &StreamId) -> usize {
        let mut client = self.redis.direct_pool.get().await.unwrap();
        redis::cmd("XLEN")
            .arg(Self::key(stream_id))
            .query_async(&mut *client)
            .await
            .unwrap()
    }

    /// The inbox is trimmed asynchronously; waits until it holds no entries.
    pub async fn wait_empty(&self, stream_id: &StreamId) {
        let result = timeout(WAIT_TIMEOUT, async {
            while self.len(stream_id).await > 0 {
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(
            result.is_ok(),
            "the inbox still holds {} entries",
            self.len(stream_id).await
        );
    }

    /// Seconds to the expiry of the inbox; negative when it has none or does not exist.
    pub async fn ttl(&self, stream_id: &StreamId) -> i64 {
        self.redis.ttl(&Self::key(stream_id)).await
    }

    pub async fn set_ttl(&self, stream_id: &StreamId, seconds: u64) {
        self.redis.expire(&Self::key(stream_id), seconds).await;
    }

    /// Listens on the inbox's wake channel. The channel is shared by every stream (and every test).
    pub async fn listen(&self) -> TestInboxWakes {
        let (tx, rx) = mpsc::unbounded_channel();
        self.db
            .listen(move |wake| {
                let _ = tx.send(wake);
            })
            .await
            .unwrap();
        TestInboxWakes { rx }
    }

    fn key(stream_id: &StreamId) -> String {
        RedisCommandInboxDb::stream_key(stream_id)
    }

    async fn xadd<T: redis::FromRedisValue>(&self, stream_id: &StreamId, at: &str, fields: &[(&str, &str)]) -> T {
        let mut client = self.redis.direct_pool.get().await.unwrap();
        let mut cmd = redis::cmd("XADD");
        cmd.arg(Self::key(stream_id)).arg(at);
        for (name, value) in fields {
            cmd.arg(*name).arg(*value);
        }
        cmd.query_async(&mut *client).await.unwrap()
    }
}

/// The wakes received from [`TestInbox::listen`].
pub struct TestInboxWakes {
    rx: mpsc::UnboundedReceiver<InboxWake>,
}

impl TestInboxWakes {
    /// The next wake of `stream_id`; wakes of other streams are skipped.
    pub async fn next_of(&mut self, stream_id: &StreamId) -> InboxWake {
        timeout(WAIT_TIMEOUT, async {
            loop {
                let wake = self.rx.recv().await.expect("the listener is open");
                match &wake {
                    InboxWake::Stream { stream_id: woken, .. } if woken == stream_id => return wake,
                    _ => {}
                }
            }
        })
        .await
        .expect("a wake of the stream")
    }
}
