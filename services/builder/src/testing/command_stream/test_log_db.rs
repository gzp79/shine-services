use crate::repositories::{
    command_log::{pg::PgCommandLogDb, CommandLogDb, StoredCommand},
    migrate_postgres,
};
use shine_game::replica::StreamId;
use shine_infra_testing::db::postgres::TestPg;
use std::time::Duration;
use tokio::time::{sleep, timeout};

const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct TestLog {
    pub pg: TestPg,
    pub db: PgCommandLogDb,
}

impl TestLog {
    pub async fn new(pg: TestPg) -> TestLog {
        migrate_postgres(&pg.pool).await.unwrap(); // fixme: it should not be here
        let db = PgCommandLogDb::new(&pg.pool).await.unwrap();
        TestLog { pg, db }
    }

    /// A command log on the test Postgres. Tests share the database and stay apart by using a stream of
    /// their own.
    pub async fn from_env() -> TestLog {
        Self::new(TestPg::from_env().await).await
    }

    /// Every command stored for the stream.
    pub async fn all_commands(&self, stream_id: &StreamId) -> Vec<StoredCommand> {
        self.db.commands_after(stream_id, 0, i64::MAX).await.unwrap()
    }

    /// The commands are persisted asynchronously; waits until the stream holds at least `count` of them.
    pub async fn wait_commands(&self, stream_id: &StreamId, count: usize) {
        let result = timeout(WAIT_TIMEOUT, async {
            while self.all_commands(stream_id).await.len() < count {
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(
            result.is_ok(),
            "{} commands persisted, expected {count}",
            self.all_commands(stream_id).await.len()
        );
    }

    /// Runs raw SQL, to damage a stream in ways the repository itself never would.
    pub async fn exec(&self, sql: &str) {
        let client = self.pg.direct_pool.get().await.unwrap();
        client.batch_execute(sql).await.unwrap();
    }

    /// Whether the database refuses this SQL.
    pub async fn rejects(&self, sql: &str) -> bool {
        let client = self.pg.direct_pool.get().await.unwrap();
        client.batch_execute(sql).await.is_err()
    }
}
