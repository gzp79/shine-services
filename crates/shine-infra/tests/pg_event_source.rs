use serde::{Deserialize, Serialize};
use shine_infra::db::{
    self,
    event_source::{self, Aggregate, Event, EventDb, EventStore, EventStoreError, SnapshotStore},
    DBError, PGConnectionPool,
};
use shine_test::test;
use std::env;
use tokio::sync::OnceCell;
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum TestEvent {
    TestEvent1 { data: String },
    TestEvent2 { aa: usize },
}

impl Event for TestEvent {
    const NAME: &'static str = "test";

    fn event_type(&self) -> &'static str {
        match self {
            TestEvent::TestEvent1 { .. } => "TestEvent1",
            TestEvent::TestEvent2 { .. } => "TestEvent2",
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TestAggregate {
    e1: String,
    aa: usize,
}

impl Aggregate for TestAggregate {
    type Event = TestEvent;
    type AggregateId = Uuid;

    const NAME: &'static str = "TestModel";

    fn apply(&mut self, event: &TestEvent) -> Result<(), EventStoreError> {
        match event {
            TestEvent::TestEvent1 { data } => {
                self.e1 += data;
            }
            TestEvent::TestEvent2 { aa } => {
                self.aa = *aa;
            }
        }
        Ok(())
    }
}

/// Initialize the test environment
static INIT: OnceCell<()> = OnceCell::const_new();
async fn initialize(cns: &str) {
    INIT.get_or_init(|| async {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let _ = create_pg_pool(cns).await.unwrap();
    })
    .await;
}

#[test]
async fn test_event_store() {
    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            initialize(&cns).await;

            let pool = create_pg_pool(&cns).await.unwrap();

            let event_db = event_source::pg::PgEventDb::<TestEvent, Uuid>::new(&pool)
                .await
                .unwrap();

            let mut es = event_db.create_context().await.unwrap();

            let aggregate = uuid::Uuid::new_v4();
            es.create_stream(&aggregate).await.unwrap();

            let e1 = TestEvent::TestEvent1 { data: "aa".to_string() };
            let e2 = TestEvent::TestEvent2 { aa: 5 };
            let e3 = TestEvent::TestEvent2 { aa: 12 };

            match es.create_stream(&aggregate).await {
                Err(EventStoreError::Conflict) => (),
                err => panic!("Expected Conflict error, {err:?}"),
            };

            es.store_events(&aggregate, None, &[e1.clone(), e2.clone()])
                .await
                .unwrap();

            match es.store_events(&aggregate, Some(1), &[e3.clone()]).await {
                Err(EventStoreError::Conflict) => (),
                err => panic!("Expected Conflict error, {err:?}"),
            };

            match es.store_events(&aggregate, Some(3), &[e3.clone()]).await {
                Err(EventStoreError::Conflict) => (),
                err => panic!("Expected Conflict error, {err:?}"),
            };

            es.store_events(&aggregate, Some(2), &[e3.clone()]).await.unwrap();

            {
                let events = es.get_events(&aggregate, Some(1), Some(2)).await.unwrap();
                log::info!("events [1..2]: {:#?}", events);
                assert_eq!(2, events.len());
                assert_eq!(1, events[0].version);
                assert_eq!(&e1, &events[0].event);
                assert_eq!(2, events[1].version);
                assert_eq!(&e2, &events[1].event);
            }

            {
                let events = es.get_events(&aggregate, None, None).await.unwrap();
                log::info!("all events: {:#?}", events);
                assert_eq!(3, events.len());
                assert_eq!(1, events[0].version);
                assert_eq!(&e1, &events[0].event);
                assert_eq!(2, events[1].version);
                assert_eq!(&e2, &events[1].event);
                assert_eq!(3, events[2].version);
                assert_eq!(&e3, &events[2].event);
            }

            es.delete_stream(&aggregate).await.unwrap();
            assert!(!es.has_stream(&aggregate).await.unwrap());
            match es.store_events(&aggregate, None, &[e1.clone()]).await {
                Err(EventStoreError::NotFound) => (),
                other => panic!("Expected NotFound, {other:?}"),
            }
            match es.delete_stream(&aggregate).await {
                Err(EventStoreError::NotFound) => (),
                other => panic!("Expected NotFound, {other:?}"),
            }
        }
        _ => log::warn!("Skipping test_stored_statements"),
    }
}

#[test]
async fn test_event_snapshots() {
    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            initialize(&cns).await;
            let pool = create_pg_pool(&cns).await.unwrap();

            let event_db = event_source::pg::PgEventDb::<TestEvent, Uuid>::new(&pool)
                .await
                .unwrap();

            event_db
                .listen_to_stream_updates(|event| {
                    log::info!("Received event: {:#?}", event);
                })
                .await
                .unwrap();

            let mut es = event_db.create_context().await.unwrap();

            let aggregate = uuid::Uuid::new_v4();
            es.create_stream(&aggregate).await.unwrap();

            let e1 = TestEvent::TestEvent1 { data: "aa".to_string() };
            let e2 = TestEvent::TestEvent2 { aa: 5 };
            let e3 = TestEvent::TestEvent2 { aa: 12 };
            let e4 = TestEvent::TestEvent1 {
                data: "_bb".to_string(),
            };

            es.store_events(&aggregate, None, &[e1.clone(), e2.clone()])
                .await
                .unwrap();
            {
                let snapshot = es.get_snapshot::<TestAggregate>(&aggregate).await.unwrap();
                assert!(snapshot.is_none());
            }

            {
                let snapshot = es.get_aggregate::<TestAggregate>(&aggregate).await.unwrap().unwrap();
                log::info!("snapshot: {:#?}", snapshot.aggregate());
                assert_eq!(2, snapshot.version());
                assert_eq!("aa", &snapshot.aggregate().e1);
                assert_eq!(5, snapshot.aggregate().aa);

                es.store_snapshot(&snapshot).await.unwrap();

                match es.store_snapshot(&snapshot).await {
                    Err(EventStoreError::Conflict) => (),
                    other => panic!("Expected Conflict error, {other:?}"),
                };
            }

            es.store_events(&aggregate, None, &[e3.clone(), e4.clone()])
                .await
                .unwrap();

            {
                let snapshot = es.get_snapshot::<TestAggregate>(&aggregate).await.unwrap().unwrap();
                assert_eq!(2, snapshot.version());
                assert_eq!("aa", &snapshot.aggregate().e1);
                assert_eq!(5, snapshot.aggregate().aa);
            }

            {
                let snapshot = es.get_aggregate::<TestAggregate>(&aggregate).await.unwrap().unwrap();
                assert_eq!(4, snapshot.version());
                assert_eq!("aa_bb", snapshot.aggregate().e1);
                assert_eq!(12, snapshot.aggregate().aa);

                es.store_snapshot(&snapshot).await.unwrap();
            }

            es.delete_stream(&aggregate).await.unwrap();
            match es.get_aggregate::<TestAggregate>(&aggregate).await {
                Err(EventStoreError::NotFound) => (),
                other => panic!("Expected NotFound, {other:?}"),
            }
            match es.get_snapshot::<TestAggregate>(&aggregate).await {
                Err(EventStoreError::NotFound) => (),
                other => panic!("Expected NotFound, {other:?}"),
            }
        }
        _ => log::warn!("Skipping test_stored_statements"),
    }
}
