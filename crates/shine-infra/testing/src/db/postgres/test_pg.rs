use crate::db::{
    postgres::{get_postgres_cns, get_postgres_cns_direct},
    with_params,
};
use shine_infra::{
    db::postgres::{PgConnectionPool, create_postgres_pool},
    web::TEST_STAGE,
};

pub struct TestPg {
    pub pool: PgConnectionPool,
    pub direct_pool: PgConnectionPool,
}

impl TestPg {
    pub async fn from_env() -> Self {
        Self::from_env_with("").await
    }

    /// Like [`from_env`](Self::from_env), with extra connection string parameters for the pool under test, for
    /// example short timeouts so that an outage fails a call soon.
    pub async fn from_env_with(params: &str) -> Self {
        let cns = with_params(&get_postgres_cns(), params);
        let pool = create_postgres_pool(TEST_STAGE, &cns).await.unwrap();

        let cns = get_postgres_cns_direct();
        let direct_pool = create_postgres_pool(TEST_STAGE, &cns).await.unwrap();
        Self { pool, direct_pool }
    }
}
