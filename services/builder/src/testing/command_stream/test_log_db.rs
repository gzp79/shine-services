use crate::repositories::{command_log::pg::PgCommandLogDb, create_postgres_pool, DBConfig};
use shine_infra::db::postgres::{self, PgConnectionPool};
use shine_infra_testing::db::postgres::get_postgres_cns;
use tokio::sync::OnceCell;

static MIGRATED: OnceCell<()> = OnceCell::const_new();

pub struct TestLog {
    pub db: PgCommandLogDb,
    pool: PgConnectionPool,
}

impl TestLog {
    /// Runs raw SQL, to damage a stream in ways the repository itself never would.
    pub async fn exec(&self, sql: &str) {
        let client = self.pool.get().await.unwrap();
        client.batch_execute(sql).await.unwrap();
    }

    /// Whether the database refuses this SQL.
    pub async fn rejects(&self, sql: &str) -> bool {
        let client = self.pool.get().await.unwrap();
        client.batch_execute(sql).await.is_err()
    }
}

/// A command log on the test Postgres, or `None` (the test skips) when none is configured. Tests
/// share the database and stay apart by using a stream of their own.
pub async fn create_test_log() -> Option<TestLog> {
    let cns = get_postgres_cns()?;
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Concurrent runners of the same migration would race on creating its objects.
    let migrate_cns = cns.clone();
    MIGRATED
        .get_or_init(|| async move {
            let config = DBConfig {
                redis_cns: String::new(),
                sql_cns: migrate_cns,
            };
            create_postgres_pool(&config)
                .await
                .expect("migrating the test database");
        })
        .await;

    let pool = postgres::create_postgres_pool(&cns).await.unwrap();
    let db = PgCommandLogDb::new(&pool).await.unwrap();
    Some(TestLog { db, pool })
}
