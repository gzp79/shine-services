use redis::{aio::ConnectionLike, AsyncCommands, Client};
use shine_infra::db::{
    create_redis_pool,
    redis::{RedisDBError, RedisListener},
};
use shine_test::test;
use std::{collections::HashSet, env, sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, Notify},
    time::{sleep, timeout},
};

#[test(serial = "redis-listener")]
async fn test_redis_listener_pub_sub() {
    match env::var("SHINE_TEST_REDIS_CNS") {
        Ok(cns) => {
            let pool = create_redis_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();

            let received = Arc::new(Notify::new());
            let received_signal = received.clone();
            let received_payload = Arc::new(Mutex::new(None));
            let received_payload_write = received_payload.clone();

            conn.listen("shine-test-channel", move |payload| {
                let received_payload_write = received_payload_write.clone();
                let received_signal = received_signal.clone();
                let Some(payload) = payload.map(|s| s.to_string()) else {
                    return;
                };
                tokio::spawn(async move {
                    *received_payload_write.lock().await = Some(payload);
                    received_signal.notify_one();
                });
            })
            .await
            .unwrap();

            let mut publisher = pool.get().await.unwrap();
            let _: () = publisher.publish("shine-test-channel", "hello").await.unwrap();

            timeout(Duration::from_secs(5), received.notified())
                .await
                .expect("timed out waiting for the pub/sub message");

            let payload = received_payload.lock().await.clone();
            assert_eq!(payload, Some("hello".to_string()));
        }

        _ => log::warn!("Skipping test_redis_listener_pub_sub"),
    }
}

#[test(serial = "redis-listener")]
async fn test_redis_listener_shared_connection_multi_channel() {
    match env::var("SHINE_TEST_REDIS_CNS") {
        Ok(cns) => {
            let pool = create_redis_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();

            let received_a = Arc::new(Mutex::new(Vec::<String>::new()));
            let received_a_write = received_a.clone();
            let notify_a = Arc::new(Notify::new());
            let notify_a_signal = notify_a.clone();

            let received_b = Arc::new(Mutex::new(Vec::<String>::new()));
            let received_b_write = received_b.clone();
            let notify_b = Arc::new(Notify::new());
            let notify_b_signal = notify_b.clone();

            conn.listen("shine-test-channel-a", move |payload| {
                let received_a_write = received_a_write.clone();
                let notify_a_signal = notify_a_signal.clone();
                if let Some(payload) = payload {
                    let payload = payload.to_string();
                    tokio::spawn(async move {
                        received_a_write.lock().await.push(payload);
                        notify_a_signal.notify_one();
                    });
                }
            })
            .await
            .unwrap();

            conn.listen("shine-test-channel-b", move |payload| {
                let received_b_write = received_b_write.clone();
                let notify_b_signal = notify_b_signal.clone();
                if let Some(payload) = payload {
                    let payload = payload.to_string();
                    tokio::spawn(async move {
                        received_b_write.lock().await.push(payload);
                        notify_b_signal.notify_one();
                    });
                }
            })
            .await
            .unwrap();

            // Both channels are served by the same shared connection; both must still receive.
            let mut publisher = pool.get().await.unwrap();
            let _: () = publisher.publish("shine-test-channel-a", "a1").await.unwrap();
            let _: () = publisher.publish("shine-test-channel-b", "b1").await.unwrap();

            timeout(Duration::from_secs(5), notify_a.notified())
                .await
                .expect("timed out waiting for channel a message");
            timeout(Duration::from_secs(5), notify_b.notified())
                .await
                .expect("timed out waiting for channel b message");

            assert_eq!(*received_a.lock().await, vec!["a1".to_string()]);
            assert_eq!(*received_b.lock().await, vec!["b1".to_string()]);

            // Unlisten channel a; channel b must keep receiving on the same shared connection,
            // and channel a must no longer be dispatched to.
            conn.unlisten("shine-test-channel-a").await.unwrap();

            let _: () = publisher.publish("shine-test-channel-a", "a2").await.unwrap();
            let _: () = publisher.publish("shine-test-channel-b", "b2").await.unwrap();

            timeout(Duration::from_secs(5), notify_b.notified())
                .await
                .expect("timed out waiting for second channel b message");

            // Give a2 a moment to have been (not) delivered.
            sleep(Duration::from_millis(200)).await;

            assert_eq!(*received_a.lock().await, vec!["a1".to_string()]);
            assert_eq!(*received_b.lock().await, vec!["b1".to_string(), "b2".to_string()]);
        }

        _ => log::warn!("Skipping test_redis_listener_shared_connection_multi_channel"),
    }
}

/// Counts pub/sub-type client connections on the server. The listener uses a dedicated pub/sub
/// connection while the pool uses ordinary multiplexed connections, so this observes exactly the
/// listener connections (assuming no other pub/sub clients — hence the `serial` tests below).
async fn count_pubsub_clients<C: ConnectionLike + Send>(conn: &mut C) -> usize {
    let list: String = redis::cmd("CLIENT")
        .arg("LIST")
        .arg("TYPE")
        .arg("pubsub")
        .query_async(conn)
        .await
        .unwrap();
    list.lines().filter(|l| !l.trim().is_empty()).count()
}

// Regression test for the reconnect signal: killing the listener's dedicated pub/sub connection
// forces a reconnect, and every handler must be invoked with `None` so consumers can resync state
// that a message dropped during the outage would have updated. Serial: it kills ALL pub/sub clients.
#[test(serial = "redis-listener")]
async fn test_redis_listener_reconnect_signal() {
    match env::var("SHINE_TEST_REDIS_CNS") {
        Ok(cns) => {
            let pool = create_redis_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();

            let events: Arc<Mutex<Vec<Option<String>>>> = Arc::new(Mutex::new(Vec::new()));
            let events_write = events.clone();
            let notify = Arc::new(Notify::new());
            let notify_signal = notify.clone();

            conn.listen("shine-test-reconnect", move |payload| {
                let events_write = events_write.clone();
                let notify_signal = notify_signal.clone();
                let payload = payload.map(|s| s.to_string());
                tokio::spawn(async move {
                    events_write.lock().await.push(payload);
                    notify_signal.notify_one();
                });
            })
            .await
            .unwrap();

            // Give the listener a moment to establish its pub/sub connection, then kill it to force
            // a reconnect.
            let mut helper = pool.get().await.unwrap();
            for _ in 0..50 {
                if count_pubsub_clients(&mut *helper).await >= 1 {
                    break;
                }
                sleep(Duration::from_millis(100)).await;
            }
            let _: () = redis::cmd("CLIENT")
                .arg("KILL")
                .arg("TYPE")
                .arg("pubsub")
                .query_async(&mut *helper)
                .await
                .unwrap();

            timeout(Duration::from_secs(10), notify.notified())
                .await
                .expect("timed out waiting for reconnect signal");

            assert!(
                events.lock().await.contains(&None),
                "expected a None reconnect signal after the pub/sub connection was killed"
            );

            // Verify the listener recovered and still delivers notifications. Retry publishing:
            // the reconnect/re-subscribe may not have completed on the very first publish.
            let mut publisher = pool.get().await.unwrap();
            let mut delivered = false;
            for _ in 0..50 {
                let _: () = publisher
                    .publish("shine-test-reconnect", "after-reconnect")
                    .await
                    .unwrap();
                if timeout(Duration::from_millis(200), notify.notified()).await.is_ok() {
                    delivered = true;
                    break;
                }
            }
            assert!(delivered, "timed out waiting for post-reconnect notification");

            assert!(
                events.lock().await.contains(&Some("after-reconnect".to_string())),
                "expected notification after reconnect"
            );
        }

        _ => log::warn!("Skipping test_redis_listener_reconnect_signal"),
    }
}

// Regression test for the listener connection/task leak: once the last handle to the pool is
// dropped, the dedicated pub/sub connection must be torn down (not left open for the process
// lifetime). Serial: it counts pub/sub clients globally.
#[test(serial = "redis-listener")]
async fn test_redis_listener_closes_connection_on_drop() {
    match env::var("SHINE_TEST_REDIS_CNS") {
        Ok(cns) => {
            // A separate, independent pool used only to observe CLIENT LIST. It must outlive the
            // pool under test.
            let observer_pool = create_redis_pool(&cns).await.unwrap();
            let mut observer = observer_pool.get().await.unwrap();

            // Wait for any pub/sub clients left over from earlier tests to disappear so we get a
            // clean baseline.
            for _ in 0..50 {
                if count_pubsub_clients(&mut *observer).await == 0 {
                    break;
                }
                sleep(Duration::from_millis(100)).await;
            }
            let baseline = count_pubsub_clients(&mut *observer).await;

            {
                let pool = create_redis_pool(&cns).await.unwrap();
                let conn = pool.get().await.unwrap();

                conn.listen("shine-test-drop", move |_payload| {}).await.unwrap();

                // The listener's dedicated pub/sub connection must actually exist while the pool is
                // alive.
                let mut appeared = false;
                for _ in 0..50 {
                    if count_pubsub_clients(&mut *observer).await > baseline {
                        appeared = true;
                        break;
                    }
                    sleep(Duration::from_millis(100)).await;
                }
                assert!(
                    appeared,
                    "listener pub/sub connection should be alive while the pool is held"
                );

                // Drop the checked-out connection then the pool. This releases the last
                // RedisListener handle, whose Drop stops the keep-alive task and tears down the
                // pub/sub connection.
                drop(conn);
                drop(pool);
            }

            // Poll until the pub/sub connection disappears (teardown is cooperative/async).
            let mut gone = false;
            for _ in 0..50 {
                if count_pubsub_clients(&mut *observer).await <= baseline {
                    gone = true;
                    break;
                }
                sleep(Duration::from_millis(100)).await;
            }

            assert!(
                gone,
                "listener pub/sub connection was not closed after the pool was dropped"
            );
        }

        _ => log::warn!("Skipping test_redis_listener_closes_connection_on_drop"),
    }
}

// Many listen() calls fired concurrently on a fresh pool all race to open the initial pub/sub
// connection. The write lock in listen() must let exactly one win and open a single shared
// connection; every channel must still be subscribed on it and receive. Proves parallel connect is
// safe (no split-brain second connection, no lost subscriptions).
#[test(serial = "redis-listener")]
async fn test_redis_listener_parallel_connect() {
    match env::var("SHINE_TEST_REDIS_CNS") {
        Ok(cns) => {
            let pool = create_redis_pool(&cns).await.unwrap();

            const N: usize = 16;
            let notify = Arc::new(Notify::new());
            let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

            let mut tasks = Vec::new();
            for i in 0..N {
                let pool = pool.clone();
                let notify = notify.clone();
                let received = received.clone();
                tasks.push(tokio::spawn(async move {
                    let conn = pool.get().await.unwrap();
                    let channel = format!("shine-test-parallel-{i}");
                    let notify_signal = notify.clone();
                    let received_write = received.clone();
                    conn.listen(&channel, move |payload| {
                        if let Some(payload) = payload.map(|s| s.to_string()) {
                            let notify_signal = notify_signal.clone();
                            let received_write = received_write.clone();
                            tokio::spawn(async move {
                                received_write.lock().await.push(payload);
                                notify_signal.notify_one();
                            });
                        }
                    })
                    .await
                    .unwrap();
                    (channel, format!("hit-{i}"))
                }));
            }

            let mut channels = Vec::new();
            for task in tasks {
                channels.push(task.await.unwrap());
            }

            // Give the shared subscription a moment to settle before publishing.
            sleep(Duration::from_millis(200)).await;

            let mut publisher = pool.get().await.unwrap();
            for (channel, expected) in &channels {
                let _: () = publisher.publish(channel, expected).await.unwrap();
            }

            let expected: HashSet<String> = channels.iter().map(|(_, e)| e.clone()).collect();
            let ok = timeout(Duration::from_secs(10), async {
                while received.lock().await.len() < expected.len() {
                    notify.notified().await;
                }
            })
            .await;

            assert!(ok.is_ok(), "timed out; received {:?}", *received.lock().await);
            assert_eq!(received.lock().await.iter().cloned().collect::<HashSet<_>>(), expected);
        }

        _ => log::warn!("Skipping test_redis_listener_parallel_connect"),
    }
}

// listen() after close() must be rejected instead of resurrecting an unmanaged connection whose
// streaming task would never self-heal. The guard returns before any I/O, so no server is needed.
#[test(serial = "redis-listener")]
async fn test_redis_listener_listen_after_close_is_rejected() {
    let client = Client::open("redis://127.0.0.1:6379").unwrap();
    let listener = RedisListener::new(client, Duration::from_secs(5), Duration::from_millis(500));
    listener.close().await;

    let err = listener.listen("shine-test-after-close", |_| {}).await.unwrap_err();
    assert!(
        matches!(err, RedisDBError::ListenerClosed),
        "expected RedisDBError::ListenerClosed, got {err:?}"
    );
}

// listen() against an unreachable broker must fail within the connect timeout rather than blocking
// on the untimed pub/sub connect, and close() must not be held up behind it (the network I/O runs
// off the state lock). Uses a black-hole address (TEST-NET-1, RFC 5737) so the connect never
// completes; no server needed.
#[test(serial = "redis-listener")]
async fn test_redis_listener_listen_times_out_when_unreachable() {
    let client = Client::open("redis://192.0.2.1:6379").unwrap();
    let connect_timeout = Duration::from_millis(300);
    let listener = RedisListener::new(client, connect_timeout, Duration::from_millis(500));

    // listen() must return an error promptly, not hang for the OS TCP timeout.
    let started = std::time::Instant::now();
    let err = timeout(
        Duration::from_secs(3),
        listener.listen("shine-test-unreachable", |_| {}),
    )
    .await
    .expect("listen() hung past the connect timeout")
    .unwrap_err();
    assert!(
        matches!(err, RedisDBError::RedisError(_)),
        "expected a Redis error from the timed-out connect, got {err:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "listen() took {:?}, expected roughly the {connect_timeout:?} connect timeout",
        started.elapsed()
    );

    // close() must not be blocked behind the stalled connect.
    timeout(Duration::from_secs(3), listener.close())
        .await
        .expect("close() was blocked behind the stalled connect");
}

// A second listen() on an already-registered channel must be rejected, not silently replace the
// handler. Needs a server: the first listen() opens the shared connection.
#[test(serial = "redis-listener")]
async fn test_redis_listener_duplicate_listen_is_rejected() {
    match env::var("SHINE_TEST_REDIS_CNS") {
        Ok(cns) => {
            let pool = create_redis_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();

            conn.listen("shine-test-duplicate", |_| {}).await.unwrap();
            let err = conn.listen("shine-test-duplicate", |_| {}).await.unwrap_err();
            assert!(
                matches!(err, RedisDBError::AlreadyListening(ref channel) if channel == "shine-test-duplicate"),
                "expected RedisDBError::AlreadyListening, got {err:?}"
            );
        }

        _ => log::warn!("Skipping test_redis_listener_duplicate_listen_is_rejected"),
    }
}
