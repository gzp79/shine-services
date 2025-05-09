use itertools::{assert_equal, Itertools};
use rand::Rng;
use serde::{Deserialize, Serialize};
use shine_infra::db::{
    self,
    event_source::{
        pg::PgEventDb, Aggregate, Event, EventDb, EventNotification, EventStore, EventStoreError, SnapshotStore,
    },
    DBError, PGConnectionPool,
};
use shine_test::test;
use std::{
    env, iter,
    ops::Deref,
    sync::{atomic::AtomicUsize, Arc},
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
    TestEvent1 { str: String },
    TestEvent2 { num: usize },
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
    str_sum: String,
    num_sum: usize,
}

impl TestAggregate {
    pub fn new(a: usize) -> Self {
        Self {
            str_sum: format!("sum_{a}"),
            num_sum: a,
        }
    }
}

impl Aggregate for TestAggregate {
    type Event = TestEvent;
    type AggregateId = Uuid;

    const NAME: &'static str = "TestModel";

    fn apply(&mut self, event: TestEvent) -> Result<(), EventStoreError> {
        match event {
            TestEvent::TestEvent1 { str } => {
                self.str_sum += &str;
            }
            TestEvent::TestEvent2 { num } => {
                self.num_sum += num;
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
async fn test_store_events() {
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

    let aggregate_id = uuid::Uuid::new_v4();

    let received_events = Arc::new(Mutex::new(Vec::new()));
    {
        let received_events = received_events.clone();
        event_db
            .listen_to_stream_updates(move |event| {
                if event.aggregate_id() != &aggregate_id {
                    return;
                }

                let received_events = received_events.clone();
                tokio::task::block_in_place(move || {
                    tokio::runtime::Handle::current().block_on(async move {
                        // emulate a slow consumer
                        tokio::time::sleep(std::time::Duration::from_micros(500)).await;
                        received_events.lock().await.push(event);
                    })
                });
            })
            .await
            .unwrap();
    }

    let mut es = event_db.create_context().await.unwrap();

    let events = [
        TestEvent::TestEvent1 { str: "1".into() },
        TestEvent::TestEvent2 { num: 2 },
        TestEvent::TestEvent1 { str: "3".to_string() },
        TestEvent::TestEvent2 { num: 4 },
        TestEvent::TestEvent2 { num: 5 },
    ];
    log::info!("Creating aggregate: {aggregate_id}...");

    // store_events should fail if the stream does not exist
    match es.store_events(&aggregate_id, 0, &events[0..0]).await {
        Err(EventStoreError::AggregateNotFound) => (),
        err => panic!("Unexpected error: {err:?}"),
    };

    es.create_stream(&aggregate_id).await.unwrap();
    match es.create_stream(&aggregate_id).await {
        Err(EventStoreError::Conflict) => (),
        err => panic!("Unexpected error: {err:?}"),
    };

    // wait for the create event to be received
    {
        let instant = std::time::Instant::now();
        while instant.elapsed().as_secs() < 2 && received_events.lock().await.is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let received_events = received_events.lock().await;
        assert_equal(
            received_events.deref(),
            &[EventNotification::StreamCreated { aggregate_id, version: 0 }],
        );
    }

    let version = es.store_events(&aggregate_id, 0, &events[0..1]).await.unwrap();
    assert_eq!(version, 1);

    // wait for the update event to be received
    {
        let instant = std::time::Instant::now();
        while instant.elapsed().as_secs() < 2 && received_events.lock().await.len() < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let received_events = received_events.lock().await;
        assert_equal(
            received_events.deref(),
            &[
                EventNotification::StreamCreated { aggregate_id, version: 0 },
                EventNotification::StreamUpdated { aggregate_id, version: 1 },
            ],
        );
    }

    match es.store_events(&aggregate_id, 0, &events[0..0]).await {
        Err(EventStoreError::Conflict) => (),
        err => panic!("Unexpected error: {err:?}"),
    };
    match es.store_events(&aggregate_id, 0, &events[0..1]).await {
        Err(EventStoreError::Conflict) => (),
        err => panic!("Unexpected error: {err:?}"),
    };
    match es.store_events(&aggregate_id, 2, &events[0..1]).await {
        Err(EventStoreError::Conflict) => (),
        err => panic!("Unexpected error: {err:?}"),
    };

    let version = es.store_events(&aggregate_id, 1, &events[1..]).await.unwrap();
    assert_eq!(version, 5);

    {
        let stored_events = es.get_events(&aggregate_id, Some(1), Some(2)).await.unwrap();
        assert_equal(stored_events.iter().map(|e| e.version), 1..=2);
        assert_equal(stored_events.iter().map(|e| &e.event), events[0..2].iter());
    }

    {
        let stored_events = es.get_events(&aggregate_id, None, None).await.unwrap();
        assert_equal(stored_events.iter().map(|e| e.version), 1..=events.len());
        assert_equal(stored_events.iter().map(|e| &e.event), events.iter());
    }

    es.delete_stream(&aggregate_id).await.unwrap();
    assert!(es.get_stream_version(&aggregate_id).await.unwrap().is_none());
    match es.store_events(&aggregate_id, 0, &events[0..0]).await {
        Err(EventStoreError::AggregateNotFound) => (),
        other => panic!("Expected NotFound, {other:?}"),
    }
    match es.store_events(&aggregate_id, 0, &events[0..1]).await {
        Err(EventStoreError::AggregateNotFound) => (),
        other => panic!("Expected NotFound, {other:?}"),
    }
    match es.delete_stream(&aggregate_id).await {
        Err(EventStoreError::AggregateNotFound) => (),
        other => panic!("Expected NotFound, {other:?}"),
    }

    //give some time for events to arrive
    let instant = std::time::Instant::now();
    while instant.elapsed().as_secs() < 4 && received_events.lock().await.len() != 4 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    {
        let received_events = received_events.lock().await;
        // events are stored in batched in a transaction, thus we one event per batch
        assert_equal(
            received_events.deref(),
            &[
                // creation and first update are different operation, thus we start by 0
                EventNotification::StreamCreated { aggregate_id, version: 0 },
                EventNotification::StreamUpdated { aggregate_id, version: 1 },
                EventNotification::StreamUpdated { aggregate_id, version: 5 },
                EventNotification::StreamDeleted { aggregate_id },
            ],
        );
    }
}

#[test(skip = "stress test, too expensive")]
async fn test_store_events_stress() {
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

    let aggregate_id = uuid::Uuid::new_v4();
    let mut es = event_db.create_context().await.unwrap();

    log::info!("Creating aggregate: {aggregate_id}...");
    es.create_stream(&aggregate_id).await.unwrap();

    const BATCH_SIZE: usize = 1000;
    const BATCH_COUNT: usize = 1000;
    const SINGLE_COUNT: usize = 10;

    // store events in batch
    let mut batch_times = Vec::new();
    let mut version = 0;
    for i in 0..BATCH_COUNT {
        let events = (0..BATCH_SIZE)
            .map(|i| TestEvent::TestEvent2 { num: i })
            .collect::<Vec<_>>();
        let instant = std::time::Instant::now();
        version = es.store_events(&aggregate_id, version, &events).await.unwrap();
        let duration = instant.elapsed();
        log::debug!("({i}) Stored {} events in {:?}", events.len(), duration);
        batch_times.push(duration.as_micros());
    }
    log::info!(
        "times: {}",
        batch_times
            .iter()
            .enumerate()
            .map(|(i, t)| format!("({}, {:?})", i, t))
            .collect::<Vec<_>>()
            .join("; ")
    );
    log::info!(
        "{{\n \"x\": [{}],\n \"y\": [{}]\n}}",
        (0..batch_times.len()).map(|i| format!("{i}")).join(","),
        batch_times.iter().map(|t| format!("{t}")).join(",")
    );

    let instant = std::time::Instant::now();
    for i in 0..SINGLE_COUNT {
        let instant = std::time::Instant::now();
        version = es
            .store_events(&aggregate_id, version, &[TestEvent::TestEvent2 { num: 42 }])
            .await
            .unwrap();
        log::debug!("({i}) Stored one more events in {:?}", instant.elapsed());
    }
    log::info!("Stored one more events in {:?}", instant.elapsed() / 10);

    let instant = std::time::Instant::now();
    for i in 0..SINGLE_COUNT {
        let instant = std::time::Instant::now();
        es.unchecked_store_events(&aggregate_id, &[TestEvent::TestEvent2 { num: 42 }])
            .await
            .unwrap();
        log::info!("({i}) Unchecked stored one more events in {:?}", instant.elapsed());
    }
    log::info!("Unchecked stored one more events in {:?}", instant.elapsed() / 10);

    let instant = std::time::Instant::now();
    es.delete_stream(&aggregate_id).await.unwrap();
    log::info!("Deleted {} events in {:?}", BATCH_COUNT * BATCH_SIZE, instant.elapsed());
}

#[test]
async fn test_unchecked_store_events() {
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

    let aggregate_id = uuid::Uuid::new_v4();

    let received_events = Arc::new(Mutex::new(Vec::new()));
    {
        let received_events = received_events.clone();
        event_db
            .listen_to_stream_updates(move |event| {
                if event.aggregate_id() != &aggregate_id {
                    return;
                }

                let received_events = received_events.clone();
                tokio::task::block_in_place(move || {
                    tokio::runtime::Handle::current().block_on(async move {
                        // emulate a slow consumer
                        tokio::time::sleep(std::time::Duration::from_micros(500)).await;
                        received_events.lock().await.push(event);
                    })
                });
            })
            .await
            .unwrap();
    }

    let mut es = event_db.create_context().await.unwrap();

    let events = [
        TestEvent::TestEvent1 { str: "1".into() },
        TestEvent::TestEvent2 { num: 2 },
        TestEvent::TestEvent1 { str: "3".to_string() },
        TestEvent::TestEvent2 { num: 4 },
        TestEvent::TestEvent2 { num: 5 },
    ];
    log::info!("Creating aggregate: {aggregate_id}...");

    let mut version = es.unchecked_store_events(&aggregate_id, &[]).await.unwrap();
    assert_eq!(version, 0);

    version = es.unchecked_store_events(&aggregate_id, &events[0..3]).await.unwrap();
    assert_eq!(version, 3);

    version = es.unchecked_store_events(&aggregate_id, &events[3..]).await.unwrap();
    assert_eq!(version, 5);

    version = es.unchecked_store_events(&aggregate_id, &[]).await.unwrap();
    assert_eq!(version, 5);

    {
        let stored_events = es.get_events(&aggregate_id, Some(1), Some(2)).await.unwrap();
        assert_equal(stored_events.iter().map(|e| e.version), 1..=2);
        assert_equal(stored_events.iter().map(|e| &e.event), events[0..2].iter());
    }

    {
        let stored_events = es.get_events(&aggregate_id, None, None).await.unwrap();
        assert_equal(stored_events.iter().map(|e| e.version), 1..=events.len());
        assert_equal(stored_events.iter().map(|e| &e.event), events.iter());
    }

    es.delete_stream(&aggregate_id).await.unwrap();

    //give some time for events to arrive
    let instant = std::time::Instant::now();
    while instant.elapsed().as_secs() < 4 && received_events.lock().await.len() != events.len() + 2 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    {
        let received_events = received_events.lock().await;
        // events are stored one-by-one, thus we should get a notification for each event
        assert_equal(
            received_events.deref(),
            // creation and first update is a single operation, thus we start by 1 (instead of the usual 0)
            &iter::once(EventNotification::StreamCreated { aggregate_id, version: 1 })
                .chain((2..=events.len()).map(|v| EventNotification::StreamUpdated { aggregate_id, version: v }))
                .chain(iter::once(EventNotification::StreamDeleted { aggregate_id }))
                .collect::<Vec<_>>(),
        );
    }
}

#[test]
async fn test_store_snapshot() {
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
    let mut es = event_db.create_context().await.unwrap();

    let aggregate_id = uuid::Uuid::new_v4();
    let events = [
        TestEvent::TestEvent1 { str: "1".into() },
        TestEvent::TestEvent2 { num: 2 },
        TestEvent::TestEvent1 { str: "3".to_string() },
        TestEvent::TestEvent2 { num: 4 },
        TestEvent::TestEvent2 { num: 5 },
    ];
    log::info!("Creating aggregate: {aggregate_id}...");

    es.create_stream(&aggregate_id).await.unwrap();

    es.unchecked_store_events(&aggregate_id, &events[0..3]).await.unwrap();

    {
        let snapshot = es.get_snapshot::<TestAggregate>(&aggregate_id, None).await.unwrap();
        assert!(snapshot.is_none());
    }

    {
        let snapshot = es
            .get_aggregate::<TestAggregate, _>(&aggregate_id, Default::default)
            .await
            .unwrap();
        assert_eq!(0, snapshot.start_version);
        assert_eq!(3, snapshot.version);
        assert_eq!("13", &snapshot.aggregate.str_sum);
        assert_eq!(2, snapshot.aggregate.num_sum);

        es.store_snapshot(&aggregate_id, 0, 3, &snapshot.aggregate)
            .await
            .unwrap();
    }

    es.unchecked_store_events(&aggregate_id, &events[3..5]).await.unwrap();

    {
        let snapshot = es
            .get_snapshot::<TestAggregate>(&aggregate_id, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(0, snapshot.start_version);
        assert_eq!(3, snapshot.version);
        assert_eq!("13", &snapshot.aggregate.str_sum);
        assert_eq!(2, snapshot.aggregate.num_sum);
    }

    {
        let snapshot = es
            .get_aggregate::<TestAggregate, _>(&aggregate_id, Default::default)
            .await
            .unwrap();
        log::info!("snapshot: {:#?}", snapshot.aggregate);
        assert_eq!(3, snapshot.start_version);
        assert_eq!(5, snapshot.version);
        assert_eq!("13", &snapshot.aggregate.str_sum);
        assert_eq!(11, snapshot.aggregate.num_sum);

        es.store_snapshot(&aggregate_id, 3, 5, &snapshot.aggregate)
            .await
            .unwrap();
    }

    {
        let snapshot = es
            .get_snapshot::<TestAggregate>(&aggregate_id, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(3, snapshot.start_version);
        assert_eq!(5, snapshot.version);
        assert_eq!("13", &snapshot.aggregate.str_sum);
        assert_eq!(11, snapshot.aggregate.num_sum);
    }

    {
        let snapshot = es
            .get_aggregate::<TestAggregate, _>(&aggregate_id, Default::default)
            .await
            .unwrap();
        assert_eq!(5, snapshot.start_version);
        assert_eq!(5, snapshot.version);
        assert_eq!("13", &snapshot.aggregate.str_sum);
        assert_eq!(11, snapshot.aggregate.num_sum);
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
async fn test_snapshot_chain() {
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
            &range.map(|i| TestEvent::TestEvent2 { num: i }).collect::<Vec<_>>(),
        )
        .await
        .unwrap();

        // parent, version,
        //  0,   3
        //  3,   5,
        //  5,   9
        es.store_snapshot(&aggregate_id, root_id, root_id + 3, &TestAggregate::new(1))
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

    let aggregate_id = uuid::Uuid::new_v4();
    log::info!("Creating aggregate: {aggregate_id}...");

    let received_events = Arc::new(Mutex::new(Vec::new()));
    {
        let received_events = received_events.clone();
        event_db
            .listen_to_stream_updates(move |event| {
                log::info!("Received event: {event:?}");
                if event.aggregate_id() != &aggregate_id {
                    return;
                }

                let received_events = received_events.clone();
                tokio::task::block_in_place(move || {
                    tokio::runtime::Handle::current().block_on(async move {
                        // emulate a slow consumer
                        tokio::time::sleep(std::time::Duration::from_micros(500)).await;
                        received_events.lock().await.push(event);
                    })
                });
            })
            .await
            .unwrap();
    }

    let mut es = event_db.create_context().await.unwrap();
    let events = (0..10).map(|i| TestEvent::TestEvent2 { num: i }).collect::<Vec<_>>();
    es.unchecked_store_events(&aggregate_id, &events).await.unwrap();

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

    //give some time for events to arrive
    let instant = std::time::Instant::now();
    while instant.elapsed().as_secs() < 4 && received_events.lock().await.len() < events.len() + 7 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    {
        let received_events = received_events.lock().await;
        // events are stored one-by-one, thus we should get a notification for each event
        assert_equal(
            received_events.deref(),
            // creation and first update is a single operation, thus we start by 1 (instead of the usual 0)
            &iter::once(EventNotification::StreamCreated { aggregate_id, version: 1 })
                .chain((2..=events.len()).map(|v| EventNotification::StreamUpdated { aggregate_id, version: v }))
                .chain([
                    EventNotification::SnapshotCreated {
                        aggregate_id,
                        snapshot: TestAggregate::NAME.into(),
                        version: 3,
                    },
                    EventNotification::SnapshotCreated {
                        aggregate_id,
                        snapshot: TestAggregate::NAME.into(),
                        version: 5,
                    },
                    EventNotification::SnapshotCreated {
                        aggregate_id,
                        snapshot: TestAggregate::NAME.into(),
                        version: 9,
                    },
                    EventNotification::SnapshotDeleted {
                        aggregate_id,
                        snapshot: TestAggregate::NAME.into(),
                        version: 3,
                    },
                    EventNotification::SnapshotDeleted {
                        aggregate_id,
                        snapshot: TestAggregate::NAME.into(),
                        version: 5,
                    },
                    EventNotification::SnapshotDeleted {
                        aggregate_id,
                        snapshot: TestAggregate::NAME.into(),
                        version: 9,
                    },
                ])
                .chain(Some(EventNotification::StreamDeleted { aggregate_id }))
                .collect::<Vec<_>>(),
        );
    }
}

#[test]
async fn test_concurrent_store_events() {
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

    let aggregate_id = uuid::Uuid::new_v4();
    log::info!("Creating aggregate: {aggregate_id}...");

    let mut es = event_db.create_context().await.unwrap();
    es.create_stream(&aggregate_id).await.unwrap();

    let num_insert = 5;
    let num_insert_unchecked = 5;
    let max_num = 1000;
    let last_num = Arc::new(AtomicUsize::new(0));
    let counts = Arc::new(Mutex::new(Vec::new()));
    let barrier = Arc::new(Barrier::new(num_insert + num_insert_unchecked + 1)); // wait far all the insertion threads

    for i in 0..num_insert {
        let event_db = event_db.clone();
        let last_num = last_num.clone();
        let counts = counts.clone();
        let barrier = barrier.clone();

        tokio::spawn(async move {
            let mut count = 0;
            let mut retry = 0;
            loop {
                let num = last_num.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if num >= max_num {
                    break;
                }

                loop {
                    retry += 1;
                    let mut es = event_db.create_context().await.unwrap();
                    let version = es.get_stream_version(&aggregate_id).await.unwrap().unwrap();
                    match es
                        .store_events(&aggregate_id, version, &[TestEvent::TestEvent2 { num }])
                        .await
                    {
                        Ok(new_version) => {
                            log::debug!("Number {num:?} stored at version {version:?}.");
                            assert_eq!(new_version, version + 1);
                            break;
                        }
                        Err(EventStoreError::Conflict) => {
                            log::debug!("Number {num:?} store failed, retry");
                            tokio::time::sleep(std::time::Duration::from_micros(100)).await;
                            continue;
                        }
                        Err(err) => {
                            panic!("Error storing number {num} at version {version}: {err:?}");
                        }
                    };
                }

                count += 1;
            }
            log::info!("Insert task {i} completed with {count} insertions an {retry} retries.");
            counts.lock().await.push(count);
            barrier.wait().await;
        });
    }

    for i in 0..num_insert_unchecked {
        let event_db = event_db.clone();
        let last_num = last_num.clone();
        let counts = counts.clone();
        let barrier = barrier.clone();

        tokio::spawn(async move {
            let mut count = 0;
            loop {
                let num = last_num.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if num >= max_num {
                    break;
                }

                let mut es = event_db.create_context().await.unwrap();
                es.unchecked_store_events(&aggregate_id, &[TestEvent::TestEvent2 { num }])
                    .await
                    .unwrap();

                count += 1;
            }
            log::info!("Unchecked insert task {i} completed with {count} insertions.");
            counts.lock().await.push(count);
            barrier.wait().await;
        });
    }

    log::info!("Waiting tasks to complete...");
    barrier.wait().await;

    let mut es = event_db.create_context().await.unwrap();

    assert_eq!(counts.lock().await.iter().sum::<usize>(), max_num);

    let version = es.get_stream_version(&aggregate_id).await.unwrap().unwrap();
    assert_eq!(version, max_num);

    // event may get inserted in out of order, but all version should be inserted exactly once
    let events = es.get_events(&aggregate_id, None, None).await.unwrap();
    let mut num_list = events
        .into_iter()
        .map(|e| match e.event {
            TestEvent::TestEvent2 { num } => num,
            _ => panic!("Unexpected event type"),
        })
        .collect::<Vec<_>>();
    num_list.sort();
    assert_equal(num_list, 0..max_num);

    log::info!("Cleaning up...");
    es.delete_stream(&aggregate_id).await.unwrap();
}

#[test]
async fn test_concurrent_snapshots_operation() {
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
    let version_gap = 3;
    let max_version = es
        .unchecked_store_events(
            &aggregate_id,
            &(0..100).map(|i| TestEvent::TestEvent2 { num: i }).collect::<Vec<_>>(),
        )
        .await
        .unwrap();

    #[derive(Debug)]
    #[allow(dead_code)]
    enum Log {
        Insert(usize, usize, usize),
        Inserted(usize, usize, usize),
        Delete(usize, usize),
        Deleted(usize, usize),
    }

    let num_insert = 3;
    let num_delete = 2;
    let op_log = Arc::new(Mutex::new(Vec::new()));
    let barrier = Arc::new(Barrier::new(num_insert + num_delete + 1)); // wait far all the insertion threads

    // Tasks to insert snapshots
    for i in 0..num_insert {
        let event_db = event_db.clone();
        let op_log = op_log.clone();
        let barrier = barrier.clone();

        tokio::spawn(async move {
            loop {
                let last_version = op_log
                    .lock()
                    .await
                    .iter()
                    .map(|d| match d {
                        // use after log as inserting duplicate is fine, but missing a completed insertion is not
                        Log::Inserted(_, _, v) => *v,
                        _ => 0,
                    })
                    .max()
                    .unwrap_or(0);

                let start = last_version;
                let version = last_version + version_gap;
                let data = TestAggregate::new(start);

                if version >= max_version {
                    break;
                }

                let mut es = event_db.create_context().await.unwrap();
                op_log.lock().await.push(Log::Insert(i, start, version));
                match es.store_snapshot(&aggregate_id, start, version, &data).await {
                    Ok(_) => log::debug!("Snapshot {:?} stored.", version),
                    Err(EventStoreError::Conflict) => {
                        assert!(
                            op_log.lock().await.iter().any(|d| match d {
                                // use before log as after event might have not been stored yet, may produce false negative
                                Log::Insert(_, s, v) => *s == start && *v == version,
                                _ => false,
                            }),
                            "Store failed for: {start},{version}\n  log: {:?}",
                            op_log.lock().await
                        );
                    }
                    Err(EventStoreError::InvalidSnapshotVersion(_, _)) => {
                        assert!(
                            // use before log as after event might have not been stored yet, may produce false negative
                            op_log.lock().await.iter().any(|d| match d {
                                Log::Delete(_, v) => *v >= start,
                                _ => false,
                            }),
                            "Store failed for: {start},{version}\n  log: {:?}",
                            op_log.lock().await,
                        );
                    }
                    Err(err) => {
                        panic!(
                            "Error storing snapshot {start},{version}\n  log: {:?}\n  err: {err:?}",
                            op_log.lock().await,
                        );
                    }
                };
                op_log.lock().await.push(Log::Inserted(i, start, version));
                tokio::task::yield_now().await;
            }
            log::info!("Insert task {i} completed.");
            barrier.wait().await;
        });
    }

    // Tasks to delete a random snapshots
    for i in 0..num_delete {
        let event_db = event_db.clone();
        let op_log = op_log.clone();
        let barrier = barrier.clone();

        tokio::spawn(async move {
            loop {
                let last_version = op_log
                    .lock()
                    .await
                    .iter()
                    .map(|d| match d {
                        // use after log as duplicate delete is fine, but deleting a future (incomplete insert) version is not
                        Log::Inserted(_, _, v) => *v,
                        _ => 0,
                    })
                    .max()
                    .unwrap_or(0);
                let last_deletion = op_log
                    .lock()
                    .await
                    .iter()
                    .map(|d| match d {
                        // use before log as duplicate less safer
                        Log::Delete(_, v) => *v,
                        _ => 0,
                    })
                    .max()
                    .unwrap_or(0);

                if last_version + version_gap >= max_version {
                    break;
                }

                if last_version > last_deletion {
                    let snapshot_to_delete = rand::rng().random_range(last_deletion..last_version);
                    let mut es = event_db.create_context().await.unwrap();
                    op_log.lock().await.push(Log::Delete(i, snapshot_to_delete));
                    match es
                        .prune_snapshot::<TestAggregate>(&aggregate_id, snapshot_to_delete)
                        .await
                    {
                        Ok(_) => log::debug!("Snapshot pruned at version {:?}.", snapshot_to_delete),
                        Err(err) => {
                            panic!(
                                "Error deleting snapshot {}\n  log: {:?}\n  err: {err:?}",
                                snapshot_to_delete,
                                op_log.lock().await,
                            );
                        }
                    };
                    op_log.lock().await.push(Log::Deleted(i, snapshot_to_delete));
                }
                tokio::task::yield_now().await;
            }
            log::info!("Delete task {i} completed.");
            barrier.wait().await;
        });
    }

    log::info!("Waiting tasks to complete...");
    barrier.wait().await;

    log::info!("Cleaning up...");
    es.delete_stream(&aggregate_id).await.unwrap();
}
