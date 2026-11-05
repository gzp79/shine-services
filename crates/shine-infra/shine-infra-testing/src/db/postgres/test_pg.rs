use crate::db::postgres::{get_postgres_cns, get_postgres_cns_direct};
use shine_infra::db::postgres::{PgConnectionPool, create_postgres_pool};

pub struct TestPg {
    pub pool: PgConnectionPool,
    pub direct_pool: PgConnectionPool,
}

impl TestPg {
    pub async fn from_env() -> Self {
        let cns = get_postgres_cns().expect("SHINE_TEST_PG_CNS must be set");
        let pool = create_postgres_pool(&cns).await.unwrap();

        let cns = get_postgres_cns_direct().expect("SHINE_TEST_PG_CNS must be set");
        let direct_pool = create_postgres_pool(&cns).await.unwrap();
        Self { pool, direct_pool }
    }
}
