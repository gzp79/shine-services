use shine_infra::db::redis::{RedisConnectionPool, create_redis_pool};
use shine_test::toxiproxy::Toxiproxy;

pub struct TestRedis {
    pub pool: RedisConnectionPool,
    pub direct_pool: RedisConnectionPool,
}

impl TestRedis {
    pub async fn new(toxiproxy: &Toxiproxy) -> Self {
        let proxy = toxiproxy.proxy("postgres").await;
        let listen = proxy.listen();
        let upstream = proxy.upstream();
        tracing::info!("Using redis proxy listen: {listen}, upstream: {upstream}");

        let cns = format!("redis://{listen}?timeout=1000&pool_timeout=500");
        let pool = create_redis_pool(&cns).await.unwrap();

        let cns = format!("redis://{upstream}?timeout=1000&pool_timeout=500");
        let direct_pool = create_redis_pool(&cns).await.unwrap();
        Self { pool, direct_pool }
    }
}
