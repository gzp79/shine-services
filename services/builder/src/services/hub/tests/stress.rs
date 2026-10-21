//! The hub at scale with every background consumer running: 300 connections, the heartbeat, the session
//! check and a stream worker on 200 ms ticks, writers and reconnecting users at once, then a shutdown.

use crate::{
    models::messages::{DisconnectReason, HubMessage, StreamMessage},
    services::{
        command_stream::{
            test_utils::{add, fast_cadence, setup_with, unique_id},
            AllowAll,
        },
        hub::Consumer,
        hub_test_utils::{
            assert_consistent, barrier, connect_with_key, create_test_hub, drops, find_registry_connection, key,
            replay, seed_session, wait_stopped, wait_ticks, Egress, Rng, TestHubOptions, BARRIER_TIMEOUT,
        },
    },
};
use shine_infra::session::SessionKey;
use shine_test::test;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        atomic::{AtomicI64, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{task::JoinHandle, time::timeout};
use uuid::Uuid;

const STABLE: usize = 280;
const CHURNING: usize = 20;
const WRITERS: usize = 4;
const COMMANDS_PER_WRITER: i64 = 25;
const COMMANDS: i64 = WRITERS as i64 * COMMANDS_PER_WRITER;

/// What a connection saw of the stream: the versions it received and how far it got.
struct Progress {
    synced: AtomicUsize,
    newest: AtomicI64,
}

/// Reads a connection until the hub closes it; returns the versions it received, in order.
fn spawn_reader(mut rx: Egress, progress: Arc<Progress>) -> JoinHandle<Vec<i64>> {
    tokio::spawn(async move {
        let mut versions = vec![];
        while let Some(message) = rx.recv().await {
            let HubMessage::Stream(message) = message else { continue };
            match message {
                StreamMessage::Sync { commands, .. } => {
                    progress.synced.fetch_add(1, Ordering::Relaxed);
                    versions.extend(commands.iter().map(|c| c.version));
                }
                StreamMessage::Commands { commands, .. } => versions.extend(commands.iter().map(|c| c.version)),
                _ => {}
            }
            if let Some(newest) = versions.last() {
                progress.newest.fetch_max(*newest, Ordering::Relaxed);
            }
        }
        versions
    })
}

#[test(serial = "hub-service")]
async fn heartbeat_enabled_stress_with_300_connections() {
    let Some(streams) = setup_with(Arc::new(AllowAll), |config| {
        fast_cadence(config);
        config.sync_interval_ms = 200;
        config.persist_delay_ms = 20;
    })
    .await
    else {
        return;
    };
    let Some(mut test) = create_test_hub(TestHubOptions {
        heartbeat: Duration::from_millis(200),
        session_check: Duration::from_millis(200),
        command_streams: Some(streams.service.clone()),
        ..Default::default()
    })
    .await
    else {
        return;
    };
    let stream_id = unique_id();
    let opened = streams.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();

    // Everybody has a session, so the session check has real work and drops nobody.
    let mut stable = vec![];
    for _ in 0..STABLE {
        let (user, session_key) = (Uuid::new_v4(), key());
        seed_session(&test.redis, user, session_key).await;
        let progress = Arc::new(Progress {
            synced: AtomicUsize::new(0),
            newest: AtomicI64::new(0),
        });
        let (connection, rx) = connect_with_key(&test.hub, user, session_key, 256);
        stable.push((user, connection, progress.clone(), spawn_reader(rx, progress)));
    }
    let churning: Vec<(Uuid, SessionKey)> = (0..CHURNING).map(|_| (Uuid::new_v4(), key())).collect();
    for (user, session_key) in &churning {
        seed_session(&test.redis, *user, *session_key).await;
    }
    let mut events = barrier(&test.hub, &mut test.events).await;

    for (user, connection, _, _) in &stable {
        test.hub
            .request_subscribe_stream(*user, *connection, stream_id.clone(), "chat", None)
            .unwrap();
    }
    timeout(BARRIER_TIMEOUT, async {
        while stable
            .iter()
            .any(|(_, _, progress, _)| progress.synced.load(Ordering::Relaxed) == 0)
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("every subscriber starts with a sync");

    // Writers and reconnecting users at the same time.
    let writers: Vec<_> = (0..WRITERS)
        .map(|writer| {
            let (service, kind, stream_id) = (streams.service.clone(), opened.kind.clone(), stream_id.clone());
            tokio::spawn(async move {
                let mut rng = Rng(0x57AE55 + writer as u64);
                for i in 0..COMMANDS_PER_WRITER {
                    let command = add(&format!("w{writer}-{i}"));
                    service
                        .ingest(kind.as_ref(), &stream_id, Uuid::new_v4(), Uuid::new_v4(), &command)
                        .await
                        .unwrap();
                    tokio::time::sleep(Duration::from_millis(rng.below(5))).await;
                }
            })
        })
        .collect();
    let churners: Vec<_> = churning
        .iter()
        .map(|(user, session_key)| {
            let (hub, stream_id, user, session_key) = (test.hub.clone(), stream_id.clone(), *user, *session_key);
            tokio::spawn(async move {
                let mut readers = vec![];
                for round in 0..5 {
                    // Each connect replaces the previous connection of the user.
                    let (connection, rx) = connect_with_key(&hub, user, session_key, 256);
                    let progress = Arc::new(Progress {
                        synced: AtomicUsize::new(0),
                        newest: AtomicI64::new(0),
                    });
                    readers.push(spawn_reader(rx, progress));
                    hub.request_subscribe_stream(user, connection, stream_id.clone(), "chat", None)
                        .unwrap();
                    tokio::time::sleep(Duration::from_millis(30 + round * 10)).await;
                }
                readers
            })
        })
        .collect();
    for writer in writers {
        writer.await.unwrap();
    }
    let mut churn_readers = vec![];
    for churner in churners {
        churn_readers.extend(churner.await.unwrap());
    }

    timeout(BARRIER_TIMEOUT, async {
        while stable
            .iter()
            .any(|(_, _, progress, _)| progress.newest.load(Ordering::Relaxed) < COMMANDS)
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("every stable subscriber gets every command");

    // The consumers kept ticking with everybody connected, and dropped nobody they should not.
    wait_ticks(&test.hub, Consumer::Heartbeat, 2).await;
    wait_ticks(&test.hub, Consumer::SessionCheck, 2).await;
    events.extend(barrier(&test.hub, &mut test.events).await);
    let active = replay(&events);
    assert_eq!(
        drops(&events),
        BTreeMap::from([(DisconnectReason::Replaced, (CHURNING * 4) as u64)]),
        "only the reconnecting users lose a connection"
    );
    let users: Vec<Uuid> = stable
        .iter()
        .map(|(user, ..)| *user)
        .chain(churning.iter().map(|(u, _)| *u))
        .collect();
    assert_eq!(active.len(), STABLE + CHURNING);
    assert_consistent(&test.hub, &test.registry, users.clone(), &active).await;
    let stats = test.hub.stats().await;
    assert!(stats.heartbeat_ticks > 0 && stats.session_check_ticks > 0);

    // A clean shutdown: every socket is closed, the registry is cleaned, the stream is persisted.
    test.hub.request_shutdown().unwrap();
    wait_stopped(&test.hub).await;
    for (_, _, _, reader) in stable {
        let versions = timeout(BARRIER_TIMEOUT, reader)
            .await
            .expect("a socket stayed open")
            .unwrap();
        assert_eq!(versions, (1..=COMMANDS).collect::<Vec<_>>(), "no gap and no repeat");
    }
    for reader in churn_readers {
        timeout(BARRIER_TIMEOUT, reader)
            .await
            .expect("a socket stayed open")
            .unwrap();
    }
    let rows: HashMap<Uuid, Option<Uuid>> = {
        let mut rows = HashMap::new();
        for user in &users {
            rows.insert(*user, find_registry_connection(&test.registry, *user).await);
        }
        rows
    };
    assert!(
        rows.values().all(Option::is_none),
        "the registry rows are removed on shutdown"
    );
    timeout(BARRIER_TIMEOUT, async {
        while streams.log.head(&stream_id).await.unwrap().unwrap().version < COMMANDS {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the workers persisted every command");
}
