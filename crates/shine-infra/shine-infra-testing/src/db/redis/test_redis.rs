use crate::db::redis::{get_redis_cns, get_redis_cns_direct};
use shine_infra::db::redis::{RedisConnectionPool, create_redis_pool};

pub struct TestRedis {
    pub pool: RedisConnectionPool,
    pub direct_pool: RedisConnectionPool,
}

impl TestRedis {
    pub async fn from_env() -> Option<Self> {
        let cns = get_redis_cns()?;
        let pool = create_redis_pool(&cns).await.unwrap();

        let cns = get_redis_cns_direct()?;
        let direct_pool = create_redis_pool(&cns).await.unwrap();
        Some(Self { pool, direct_pool })
    }
}
