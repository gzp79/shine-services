use crate::repositories::command_inbox::{redis::RedisCommandInboxDb, CommandInboxDb, InboxId, InboxWake, StreamId};
use shine_infra::db::redis::{create_redis_pool, RedisConnectionPool};
use std::{env, time::Duration};
use tokio::{sync::mpsc, time::timeout};
use uuid::Uuid;

const WAKE_TIMEOUT: Duration = Duration::from_secs(5);

pub struct TestInbox {
    pub db: RedisCommandInboxDb,
    redis: RedisConnectionPool,
}

impl TestInbox {
    /// Adds a raw entry, to put in the inbox what the repository itself never would.
    pub async fn raw_add(&self, stream_id: &StreamId, fields: &[(&str, &str)]) -> InboxId {
        let mut client = self.redis.get().await.unwrap();
        let mut cmd = redis::cmd("XADD");
        cmd.arg(RedisCommandInboxDb::stream_key(stream_id)).arg("*");
        for (name, value) in fields {
            cmd.arg(*name).arg(*value);
        }
        let id: String = cmd.query_async(&mut *client).await.unwrap();
        InboxId::parse(&id).unwrap()
    }

    /// Adds a raw entry at a given position, in an inbox that has none above it (see `drop_inbox`).
    pub async fn raw_add_at(&self, stream_id: &StreamId, at: &InboxId, fields: &[(&str, &str)]) {
        let mut client = self.redis.get().await.unwrap();
        let mut cmd = redis::cmd("XADD");
        cmd.arg(RedisCommandInboxDb::stream_key(stream_id)).arg(at.to_string());
        for (name, value) in fields {
            cmd.arg(*name).arg(*value);
        }
        let _: String = cmd.query_async(&mut *client).await.unwrap();
    }

    /// Deletes the whole inbox, with the memory of its last id, as if it had expired.
    pub async fn drop_inbox(&self, stream_id: &StreamId) {
        let mut client = self.redis.get().await.unwrap();
        let _: i64 = redis::cmd("DEL")
            .arg(RedisCommandInboxDb::stream_key(stream_id))
            .query_async(&mut *client)
            .await
            .unwrap();
    }

    pub async fn len(&self, stream_id: &StreamId) -> usize {
        let mut client = self.redis.get().await.unwrap();
        redis::cmd("XLEN")
            .arg(RedisCommandInboxDb::stream_key(stream_id))
            .query_async(&mut *client)
            .await
            .unwrap()
    }

    /// Seconds to the expiry of the inbox; negative when it has none or does not exist.
    pub async fn ttl(&self, stream_id: &StreamId) -> i64 {
        let mut client = self.redis.get().await.unwrap();
        redis::cmd("TTL")
            .arg(RedisCommandInboxDb::stream_key(stream_id))
            .query_async(&mut *client)
            .await
            .unwrap()
    }

    pub async fn set_ttl(&self, stream_id: &StreamId, seconds: u64) {
        let mut client = self.redis.get().await.unwrap();
        let _: i64 = redis::cmd("EXPIRE")
            .arg(RedisCommandInboxDb::stream_key(stream_id))
            .arg(seconds)
            .query_async(&mut *client)
            .await
            .unwrap();
    }

    /// Listens on the inbox's wake channel. The channel is shared by every stream (and every test), so
    /// the receiver is used through `next_wake_of`.
    pub async fn listen(&self) -> mpsc::UnboundedReceiver<InboxWake> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.db
            .listen(move |wake| {
                let _ = tx.send(wake);
            })
            .await
            .unwrap();
        rx
    }
}

/// An inbox on the test Redis, or `None` (the test skips) when none is configured. Tests share the
/// Redis and stay apart by using a stream of their own.
pub async fn create_test_inbox(ttl_seconds: u64) -> Option<TestInbox> {
    let Some(cns) = env::var("SHINE_TEST_REDIS_CNS")
        .or_else(|_| env::var("SHINE_REDIS_CNS"))
        .ok()
    else {
        tracing::warn!("Missing SHINE_TEST_REDIS_CNS/SHINE_REDIS_CNS, skipping test");
        return None;
    };
    // A pool of its own, so it has its own wake listener.
    let redis = create_redis_pool(&cns).await.unwrap();
    let db = RedisCommandInboxDb::new(&redis, ttl_seconds).await.unwrap();
    Some(TestInbox { db, redis })
}

pub fn stream_id() -> StreamId {
    StreamId::parse(&format!("t-{}", Uuid::new_v4().simple())).unwrap()
}

/// The next wake of `stream_id`; wakes of other streams are skipped.
pub async fn next_wake_of(rx: &mut mpsc::UnboundedReceiver<InboxWake>, stream_id: &StreamId) -> InboxWake {
    timeout(WAKE_TIMEOUT, async {
        loop {
            let wake = rx.recv().await.expect("the listener is open");
            match &wake {
                InboxWake::Stream { stream_id: woken, .. } if woken == stream_id => return wake,
                _ => {}
            }
        }
    })
    .await
    .expect("a wake of the stream")
}
