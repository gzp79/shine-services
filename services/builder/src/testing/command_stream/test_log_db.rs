use crate::repositories::command_log::pg::PgCommandLogDb;
use shine_infra::db::postgres::{create_postgres_pool, PgConnectionPool};
use shine_infra_testing::db::postgres::get_postgres_cns;

pub struct TestLog {
    pub db: PgCommandLogDb,
    pub postgres: PgConnectionPool,
}

impl TestLog {
    pub fn new(postgres: PgConnectionPool, db: PgCommandLogDb) -> TestLog {
        TestLog { db, postgres }
    }

    /// A command log on the test Postgres, or `None` (the test skips) when none is configured. Tests
    /// share the database and stay apart by using a stream of their own.
    pub async fn from_env() -> Option<TestLog> {
        let cns = get_postgres_cns()?;
        let postgres = create_postgres_pool(&cns).await.unwrap();
        let db = PgCommandLogDb::new(&postgres).await.unwrap();
        Some(TestLog { db, postgres })
    }

    /// A command log on the test Postgres, or `None` (the test skips) when none is configured. Tests
    /// share the database and stay apart by using a stream of their own.
    pub async fn from_db(postgres: PgConnectionPool) -> Option<TestLog> {
        let db = PgCommandLogDb::new(&postgres).await.unwrap();
        Some(TestLog { db, postgres })
    }

    /// Runs raw SQL, to damage a stream in ways the repository itself never would.
    pub async fn exec(&self, sql: &str) {
        let client = self.postgres.get().await.unwrap();
        client.batch_execute(sql).await.unwrap();
    }

    /// Whether the database refuses this SQL.
    pub async fn rejects(&self, sql: &str) -> bool {
        let client = self.postgres.get().await.unwrap();
        client.batch_execute(sql).await.is_err()
    }
}
