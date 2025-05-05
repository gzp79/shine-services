use rand::Rng;
use serde::{Deserialize, Serialize};
use shine_infra::db::{
    self,
    event_source::{pg::PgEventDb, Aggregate, Event, EventDb, EventStore, EventStoreError, SnapshotStore},
    DBError, PGConnectionPool,
};
use shine_test::test;
use std::{
    collections::HashSet,
    env, panic, process,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::{Barrier, Mutex, OnceCell};
use uuid::Uuid;

pub async fn create_pg_pool(cns: &str) -> Result<PGConnectionPool, DBError> {
    log::info!("Creating postgres pool...");
    let postgres = db::create_postgres_pool(cns)
        .await
        .map_err(DBError::PGCreatePoolError)?;
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

impl TestAggregate {
    pub fn new(a: usize) -> Self {
        Self {
            e1: format!("aa_{a}"),
            aa: a,
        }
    }
}

impl Aggregate for TestAggregate {
    type Event = TestEvent;
    type AggregateId = Uuid;

    const NAME: &'static str = "TestModel";

    fn apply(&mut self, event: TestEvent) -> Result<(), EventStoreError> {
        match event {
            TestEvent::TestEvent1 { data } => {
                self.e1 += &data;
            }
            TestEvent::TestEvent2 { aa } => {
                self.aa = aa;
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

        let pool = create_pg_pool(cns).await.unwrap();
        let mut client = pool.get().await.unwrap();
        client
            .migrate("es_test", &PgEventDb::<TestEvent, Uuid>::migrations())
            .await
            .unwrap();
    })
    .await;
}

#[test]
async fn test_event_store() {
    let cns = match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => cns,
        Err(_) => {
            log::warn!("SHINE_TEST_PG_CNS not set, skipping test_event_store");
            return;
        }
    };
    initialize(&cns).await;

    let pool = create_pg_pool(&cns).await.unwrap();
    let event_db = PgEventDb::<TestEvent, Uuid>::new(&pool).await.unwrap();

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

    es.store_events(&aggregate, 0, &[e1.clone(), e2.clone()]).await.unwrap();

    match es.store_events(&aggregate, 1, &[e3.clone()]).await {
        Err(EventStoreError::Conflict) => (),
        err => panic!("Expected Conflict error, {err:?}"),
    };

    match es.store_events(&aggregate, 3, &[e3.clone()]).await {
        Err(EventStoreError::Conflict) => (),
        err => panic!("Expected Conflict error, {err:?}"),
    };

    es.store_events(&aggregate, 2, &[e3.clone()]).await.unwrap();

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
    match es.store_events(&aggregate, 0, &[e1.clone()]).await {
        Err(EventStoreError::AggregateNotFound) => (),
        other => panic!("Expected NotFound, {other:?}"),
    }
    match es.delete_stream(&aggregate).await {
        Err(EventStoreError::AggregateNotFound) => (),
        other => panic!("Expected NotFound, {other:?}"),
    }
}

#[test]
async fn test_unchecked_event_store() {
    let cns = match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => cns,
        Err(_) => {
            log::warn!("SHINE_TEST_PG_CNS not set, skipping test_event_store");
            return;
        }
    };
    initialize(&cns).await;

    let pool = create_pg_pool(&cns).await.unwrap();
    let event_db = PgEventDb::<TestEvent, Uuid>::new(&pool).await.unwrap();

    let mut es = event_db.create_context().await.unwrap();

    let aggregate = uuid::Uuid::new_v4();
    let e1 = TestEvent::TestEvent1 { data: "aa".to_string() };
    let e2 = TestEvent::TestEvent2 { aa: 5 };
    let e3 = TestEvent::TestEvent2 { aa: 12 };

    let mut version = es.unchecked_store_events(&aggregate, &[]).await.unwrap();
    assert_eq!(version, 0);

    version = es
        .unchecked_store_events(&aggregate, &[e1.clone(), e2.clone()])
        .await
        .unwrap();
    assert_eq!(version, 2);

    version = es.unchecked_store_events(&aggregate, &[e3.clone()]).await.unwrap();
    assert_eq!(version, 3);

    version = es.unchecked_store_events(&aggregate, &[]).await.unwrap();
    assert_eq!(version, 3);

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
}

#[test]
async fn test_event_snapshots() {
    let cns = match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => cns,
        _ => {
            log::warn!("Skipping test_stored_statements");
            return;
        }
    };

    initialize(&cns).await;
    let pool = create_pg_pool(&cns).await.unwrap();
    let event_db = PgEventDb::<TestEvent, Uuid>::new(&pool).await.unwrap();

    event_db
        .listen_to_stream_updates(|event| {
            log::info!("Received event: {:#?}", event);
        })
        .await
        .unwrap();

    let mut es = event_db.create_context().await.unwrap();

    let aggregate_id = uuid::Uuid::new_v4();
    es.create_stream(&aggregate_id).await.unwrap();

    let e1 = TestEvent::TestEvent1 { data: "aa".to_string() };
    let e2 = TestEvent::TestEvent2 { aa: 5 };
    let e3 = TestEvent::TestEvent2 { aa: 12 };
    let e4 = TestEvent::TestEvent1 {
        data: "_bb".to_string(),
    };

    es.store_events(&aggregate_id, 0, &[e1.clone(), e2.clone()])
        .await
        .unwrap();
    {
        let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, None).await.unwrap();
        assert!(snapshot.is_none());
    }

    {
        let snapshot = es
            .get_aggregate::<TestAggregate, _>(&aggregate_id, Default::default)
            .await
            .unwrap();
        log::info!("snapshot: {:#?}", snapshot.aggregate);
        assert_eq!(0, snapshot.start_version);
        assert_eq!(2, snapshot.version);
        assert_eq!("aa", &snapshot.aggregate.e1);
        assert_eq!(5, snapshot.aggregate.aa);

        es.store_snapshot(&aggregate_id, 0, 2, &snapshot.aggregate)
            .await
            .unwrap();
    }

    es.store_events(&aggregate_id, 2, &[e3.clone(), e4.clone()])
        .await
        .unwrap();

    {
        let snapshot = es
            .get_snapshot::<TestAggregate>(&aggregate_id, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(0, snapshot.start_version);
        assert_eq!(2, snapshot.version);
        assert_eq!("aa", &snapshot.aggregate.e1);
        assert_eq!(5, snapshot.aggregate.aa);
    }

    {
        let snapshot = es
            .get_aggregate::<TestAggregate, _>(&aggregate_id, Default::default)
            .await
            .unwrap();
        assert_eq!(2, snapshot.start_version);
        assert_eq!(4, snapshot.version);
        assert_eq!("aa_bb", snapshot.aggregate.e1);
        assert_eq!(12, snapshot.aggregate.aa);

        es.store_snapshot(&aggregate_id, 2, 4, &snapshot.aggregate)
            .await
            .unwrap();
    }

    es.delete_stream(&aggregate_id).await.unwrap();
    match es
        .get_aggregate::<TestAggregate, _>(&aggregate_id, Default::default)
        .await
    {
        Err(EventStoreError::AggregateNotFound) => (),
        other => panic!("Expected NotFound, {other:?}"),
    }
    match es.get_snapshot::<TestAggregate>(&aggregate_id, None).await {
        Err(EventStoreError::AggregateNotFound) => (),
        other => panic!("Expected NotFound, {other:?}"),
    }
}

#[test]
async fn test_snapshots_chain() {
    let cns = match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => cns,
        _ => {
            log::warn!("Skipping test_stored_statements");
            return;
        }
    };
    initialize(&cns).await;
    let pool = create_pg_pool(&cns).await.unwrap();
    let event_db = PgEventDb::<TestEvent, Uuid>::new(&pool).await.unwrap();

    for root_id in [0, 7] {
        let mut es = event_db.create_context().await.unwrap();
        let aggregate_id = uuid::Uuid::new_v4();

        es.create_stream(&aggregate_id).await.unwrap();

        // create some events
        let range = if root_id == 0 { 0..15 } else { 0..30 };
        es.unchecked_store_events(
            &aggregate_id,
            &range
                .map(|i| TestEvent::TestEvent1 { data: i.to_string() })
                .collect::<Vec<_>>(),
        )
        .await
        .unwrap();

        // parent, version,
        //  0,   3
        //  3,   5,
        //  5,   9
        es.store_snapshot(&aggregate_id, root_id + 0, root_id + 3, &TestAggregate::new(1))
            .await
            .unwrap();
        es.store_snapshot(&aggregate_id, root_id + 3, root_id + 5, &TestAggregate::new(2))
            .await
            .unwrap();
        es.store_snapshot(&aggregate_id, root_id + 5, root_id + 9, &TestAggregate::new(3))
            .await
            .unwrap();

        for (idx, (start, end, expected)) in [
            (0, 3, Some(EventStoreError::Conflict)),
            (0, 5, Some(EventStoreError::Conflict)),
            (3, 5, Some(EventStoreError::Conflict)),
            (5, 9, Some(EventStoreError::Conflict)),
            (0, 2, Some(EventStoreError::Conflict)),
            (3, 3, Some(EventStoreError::InvalidSnapshotVersion(3, 3))),
            (3, 4, Some(EventStoreError::Conflict)),
            (4, 3, Some(EventStoreError::InvalidSnapshotVersion(4, 3))),
            (4, 7, Some(EventStoreError::InvalidSnapshotVersion(4, 7))),
            (7, 4, Some(EventStoreError::InvalidSnapshotVersion(7, 4))),
            (5, 6, Some(EventStoreError::Conflict)),
            (3, 9, Some(EventStoreError::Conflict)),
            (9, 99, Some(EventStoreError::EventVersionNotFound(99))),
            (9, 10, None),
        ]
        .into_iter()
        .enumerate()
        {
            log::info!("Case ({}): {} ({},{})", aggregate_id, idx, start, end);
            let res = match es
                .store_snapshot(&aggregate_id, root_id + start, root_id + end, &TestAggregate::new(idx))
                .await
            {
                Err(EventStoreError::InvalidSnapshotVersion(a, b)) => {
                    Err(EventStoreError::InvalidSnapshotVersion(a - root_id, b - root_id))
                }
                Err(EventStoreError::EventVersionNotFound(a)) => {
                    Err(EventStoreError::EventVersionNotFound(a - root_id))
                }
                res => res,
            };

            let err = res.err().map(|e| format!("{:?}", e));
            let expected = expected.map(|e| format!("{:?}", e));
            assert_eq!(err, expected);
        }

        es.delete_stream(&aggregate_id).await.unwrap();
    }
}

#[test]
async fn test_prune_snapshots() {
    let cns = match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => cns,
        _ => {
            log::warn!("Skipping test_stored_statements");
            return;
        }
    };
    initialize(&cns).await;
    let pool = create_pg_pool(&cns).await.unwrap();
    let event_db = Arc::new(PgEventDb::<TestEvent, Uuid>::new(&pool).await.unwrap());
    let mut es = event_db.create_context().await.unwrap();
    let aggregate_id = uuid::Uuid::new_v4();
    es.create_stream(&aggregate_id).await.unwrap();

    log::info!("Storing events for aggregate: {aggregate_id}...");
    es.unchecked_store_events(
        &aggregate_id,
        &(0..100)
            .map(|i| TestEvent::TestEvent1 { data: i.to_string() })
            .collect::<Vec<_>>(),
    )
    .await
    .unwrap();

    // parent, version,
    //  0,   3
    //  3,   5,
    //  5,   9
    es.store_snapshot(&aggregate_id, 0, 3, &TestAggregate::new(1))
        .await
        .unwrap();
    es.store_snapshot(&aggregate_id, 3, 5, &TestAggregate::new(2))
        .await
        .unwrap();
    es.store_snapshot(&aggregate_id, 5, 9, &TestAggregate::new(3))
        .await
        .unwrap();

    let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, Some(2)).await.unwrap();
    assert!(snapshot.is_none());

    let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, Some(3)).await.unwrap();
    assert_eq!(3, snapshot.map(|s| s.version).unwrap());

    let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, Some(4)).await.unwrap();
    assert_eq!(3, snapshot.map(|s| s.version).unwrap());

    let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, Some(5)).await.unwrap();
    assert_eq!(5, snapshot.map(|s| s.version).unwrap());

    // nothing is removed
    // chain: 0,3 3,5 5,9
    es.prune_snapshot::<TestAggregate>(&aggregate_id, 2).await.unwrap();
    let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, Some(3)).await.unwrap();
    assert_eq!(3, snapshot.map(|s| s.version).unwrap());

    // remove at exact version (3)
    // chain: 0,3 3,5 5,9
    // keep: 3,5 5,9
    es.prune_snapshot::<TestAggregate>(&aggregate_id, 3).await.unwrap();
    let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, Some(3)).await.unwrap();
    assert!(snapshot.is_none());
    let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, Some(5)).await.unwrap();
    assert_eq!(5, snapshot.map(|s| s.version).unwrap());

    // remove at a version (6)
    // chain: 3,5 5,9
    // keep: 5,9
    es.prune_snapshot::<TestAggregate>(&aggregate_id, 6).await.unwrap();
    let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, Some(5)).await.unwrap();
    assert!(snapshot.is_none());
    let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, Some(9)).await.unwrap();
    assert_eq!(9, snapshot.map(|s| s.version).unwrap());

    es.prune_snapshot::<TestAggregate>(&aggregate_id, 99).await.unwrap();
    let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, Some(9)).await.unwrap();
    assert!(snapshot.is_none());
    let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, None).await.unwrap();
    assert!(snapshot.is_none());

    es.delete_stream(&aggregate_id).await.unwrap();
}

#[test]
async fn test_concurrent_snapshots_operation() {
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        process::exit(-1);
    }));

    let cns = match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => cns,
        _ => {
            log::warn!("Skipping test_stored_statements");
            return;
        }
    };
    initialize(&cns).await;
    let pool = create_pg_pool(&cns).await.unwrap();
    let event_db = Arc::new(PgEventDb::<TestEvent, Uuid>::new(&pool).await.unwrap());
    let mut es = event_db.create_context().await.unwrap();
    let aggregate_id = uuid::Uuid::new_v4();
    es.create_stream(&aggregate_id).await.unwrap();

    log::info!("Storing events for aggregate: {aggregate_id}...");
    let max_version = es
        .unchecked_store_events(
            &aggregate_id,
            &(0..100)
                .map(|i| TestEvent::TestEvent1 { data: i.to_string() })
                .collect::<Vec<_>>(),
        )
        .await
        .unwrap();

    let last_insertion = Arc::new(AtomicUsize::new(0));
    let num_insert = 3;
    let deletes = Arc::new(Mutex::new(HashSet::new()));
    let barrier = Arc::new(Barrier::new(num_insert)); // wait far all the insertion threads

    // Spawn 3 threads for inserting snapshots
    for _ in 0..num_insert {
        let event_db = event_db.clone();
        let aggregate_id = aggregate_id.clone();
        let last_insertion = last_insertion.clone();
        let deletes = deletes.clone();
        let barrier = barrier.clone();

        tokio::spawn(async move {
            loop {
                let idx = last_insertion.load(Ordering::SeqCst);

                let start = 3 * idx;
                let end = start + 3;
                let data = TestAggregate::new(idx);

                if end >= max_version {
                    break;
                }

                let mut es = event_db.create_context().await.unwrap();
                match es.store_snapshot(&aggregate_id, start, end, &data).await {
                    Ok(_) => log::debug!("Snapshot {:?} stored.", end),
                    Err(EventStoreError::Conflict) => log::trace!("Snapshot already stored: {:?}", end),
                    Err(EventStoreError::InvalidSnapshotVersion(a, b)) => {
                        log::info!(
                            "Snapshot insertion ({},{}) is behind delete. version:{}, current_version: {}, deletes: {:?}",
                            a,
                            b,
                            idx * 3,
                            last_insertion.load(Ordering::SeqCst) * 3,
                            deletes.lock().await
                        );
                        // insertion failed as snapshot chain was trimmed at a later version
                        assert!(deletes.lock().await.iter().any(|d| *d >= a));
                    }
                    Err(err) => panic!(
                        "Error storing snapshot: {err:?}, idx: {idx}, last_insertion: {:?}",
                        last_insertion.load(Ordering::SeqCst)
                    ),
                };

                last_insertion.store(idx + 1, Ordering::SeqCst);
                tokio::task::yield_now().await;
            }
            barrier.wait().await;
        });
    }

    {
        let event_db = event_db.clone();
        let aggregate_id = aggregate_id.clone();
        let last_insertion = last_insertion.clone();
        let deletes = deletes.clone();
        let barrier = barrier.clone();

        tokio::spawn(async move {
            let mut min_snapshot = 0;

            loop {
                let max_snapshot = last_insertion.load(Ordering::SeqCst) * 3;

                if max_snapshot > min_snapshot {
                    let snapshot_to_delete = rand::rng().random_range(min_snapshot..max_snapshot);
                    min_snapshot = snapshot_to_delete + 1;
                    let mut es = event_db.create_context().await.unwrap();
                    match es
                        .prune_snapshot::<TestAggregate>(&aggregate_id, snapshot_to_delete)
                        .await
                    {
                        Ok(_) => {
                            deletes.lock().await.insert(snapshot_to_delete);
                            log::debug!("Snapshot pruned at version {:?}.", snapshot_to_delete);
                        }
                        Err(err) => log::debug!("Snapshot prune failed : {:?}, err {:?}", snapshot_to_delete, err),
                    }
                }

                if max_snapshot >= max_version {
                    break;
                }
                tokio::task::yield_now().await;
            }
            barrier.wait().await;
        });
    }

    barrier.wait().await;

    //es.delete_stream(&aggregate_id).await.unwrap();
}
