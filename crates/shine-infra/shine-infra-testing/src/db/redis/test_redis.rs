use crate::db::redis::{get_redis_cns, get_redis_cns_direct};
use shine_infra::db::redis::{RedisConnectionPool, create_redis_pool};

/// Redis pools of a test: `pool` for the code under test (may go through a fault-injecting proxy) and
/// `direct_pool` for the test itself to inspect and damage the data, bypassing any proxy.
#[derive(Clone)]
pub struct TestRedis {
    pub pool: RedisConnectionPool,
    pub direct_pool: RedisConnectionPool,
}

impl TestRedis {
    pub async fn from_env() -> Self {
        let cns = get_redis_cns().expect("SHINE_TEST_REDIS_CNS must be set");
        let pool = create_redis_pool(&cns).await.unwrap();

        let cns = get_redis_cns_direct().expect("SHINE_TEST_REDIS_CNS must be set");
        let direct_pool = create_redis_pool(&cns).await.unwrap();
        Self { pool, direct_pool }
    }

    /// Deletes the key; returns whether it existed.
    pub async fn del(&self, key: &str) -> bool {
        let mut client = self.direct_pool.get().await.unwrap();
        let deleted: i64 = redis::cmd("DEL").arg(key).query_async(&mut *client).await.unwrap();
        deleted > 0
    }

    /// Sets the expiry of the key; returns whether the key exists.
    pub async fn expire(&self, key: &str, seconds: u64) -> bool {
        let mut client = self.direct_pool.get().await.unwrap();
        let updated: i64 = redis::cmd("EXPIRE")
            .arg(key)
            .arg(seconds)
            .query_async(&mut *client)
            .await
            .unwrap();
        updated > 0
    }

    /// Seconds to the expiry of the key; negative when it has none (-1) or does not exist (-2).
    pub async fn ttl(&self, key: &str) -> i64 {
        let mut client = self.direct_pool.get().await.unwrap();
        redis::cmd("TTL").arg(key).query_async(&mut *client).await.unwrap()
    }
}
