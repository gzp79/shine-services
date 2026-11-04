use crate::db::postgres::{get_postgres_cns, get_postgres_cns_direct};
use shine_infra::db::redis::{RedisConnectionPool, create_redis_pool};

pub struct TestPg {
    pub pool: RedisConnectionPool,
    pub direct_pool: RedisConnectionPool,
}

impl TestPg {
    pub async fn from_env() -> Option<Self> {
        let cns = get_postgres_cns()?;
        let pool = create_redis_pool(&cns).await.unwrap();

        let cns = get_postgres_cns_direct()?;
        let direct_pool = create_redis_pool(&cns).await.unwrap();
        Some(Self { pool, direct_pool })
    }
}
