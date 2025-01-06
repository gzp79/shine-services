use serde::{Deserialize, Serialize};
use shine_core::db::{
    self,
    event_source::{self, Event, EventStore, EventStoreError},
    DBError, PGConnectionPool,
};
use shine_test::test;
use std::env;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./tests/pg_event_source_sql_migrations");
}

pub async fn create_pg_pool(cns: &str) -> Result<PGConnectionPool, DBError> {
    log::info!("Creating postgres pool...");
    let postgres = db::create_postgres_pool(cns)
        .await
        .map_err(DBError::PGCreatePoolError)?;

    {
        log::info!("Migrating database...");
        let mut backend = postgres.get().await.map_err(DBError::PGPoolError)?;
        log::debug!("migrations: {:#?}", embedded::migrations::runner().get_migrations());
        let client = &mut **backend;
        embedded::migrations::runner().run_async(client).await?;
    }

    Ok(postgres)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum TestEvent {
    TestEvent1 { data: String },
    TestEvent2 { aa: usize },
}

impl Event for TestEvent {
    fn event_type(&self) -> &'static str {
        match self {
            TestEvent::TestEvent1{..} => "TestEvent1",
            TestEvent::TestEvent2{..} => "TestEvent2",
        }
    }
}

#[test]
async fn test_event_source() {
    rustls::crypto::ring::default_provider().install_default().unwrap();

    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            let pool = create_pg_pool(&cns).await.unwrap();

            let es = event_source::pg::PgEventStore::<TestEvent>::new(&pool, "test")
                .await
                .unwrap();

            let aggregate = uuid::Uuid::new_v4();
            es.create(&aggregate).await.unwrap();

            match es.create(&aggregate).await {
                Err(EventStoreError::Conflict) => (),
                err => panic!("Expected Conflict error, {err:?}"),
            };

            es.store(
                &aggregate,
                None,
                &[
                    TestEvent::TestEvent1 { data: "aa".to_string() },
                    TestEvent::TestEvent2 { aa: 5 },
                ],
            )
            .await
            .unwrap();

            match es
                .store(&aggregate, Some(1), &[TestEvent::TestEvent1 { data: "bb".to_string() }])
                .await
            {
                Err(EventStoreError::Conflict) => (),
                err => panic!("Expected Conflict error, {err:?}"),
            };

            match es
                .store(&aggregate, Some(3), &[TestEvent::TestEvent1 { data: "bb".to_string() }])
                .await
            {
                Err(EventStoreError::Conflict) => (),
                err => panic!("Expected Conflict error, {err:?}"),
            };

            es.store(&aggregate, Some(2), &[TestEvent::TestEvent2 { aa: 12 }])
                .await
                .unwrap();
        }
        _ => log::warn!("Skipping test_stored_statements"),
    }
}
