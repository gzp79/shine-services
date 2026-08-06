use rustls::crypto::ring;
use shine_infra::db::postgres::{create_postgres_pool, PgConfig, PgError, PgListener};
use shine_test::test;
use std::{collections::HashSet, env, str::FromStr, sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, Notify},
    time::{sleep, timeout},
};
use tokio_postgres_rustls::MakeRustlsConnect;

#[test(serial = "pg-listener")]
async fn test_pg_listener_pub_sub() {
    let _ = ring::default_provider().install_default();

    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            let pool = create_postgres_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();

            let received = Arc::new(Notify::new());
            let received_signal = received.clone();
            let received_payload = Arc::new(Mutex::new(None));
            let received_payload_write = received_payload.clone();

            conn.listen("shine-test-channel", move |payload| {
                let received_payload_write = received_payload_write.clone();
                let received_signal = received_signal.clone();
                let payload = payload.map(|s| s.to_string());
                tokio::spawn(async move {
                    *received_payload_write.lock().await = payload;
                    received_signal.notify_one();
                });
            })
            .await
            .unwrap();

            let publisher = pool.get().await.unwrap();
            publisher
                .execute("SELECT pg_notify('shine-test-channel', 'hello')", &[])
                .await
                .unwrap();

            timeout(Duration::from_secs(5), received.notified())
                .await
                .expect("timed out waiting for the pg LISTEN/NOTIFY message");

            let payload = received_payload.lock().await.clone();
            assert_eq!(payload, Some("hello".to_string()));
        }

        _ => log::warn!("Skipping test_pg_listener_pub_sub"),
    }
}

#[test(serial = "pg-listener")]
async fn test_pg_listener_shared_connection_multi_channel() {
    let _ = ring::default_provider().install_default();

    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            let pool = create_postgres_pool(&cns).await.unwrap();
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
            let publisher = pool.get().await.unwrap();
            publisher
                .execute("SELECT pg_notify('shine-test-channel-a', 'a1')", &[])
                .await
                .unwrap();
            publisher
                .execute("SELECT pg_notify('shine-test-channel-b', 'b1')", &[])
                .await
                .unwrap();

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

            publisher
                .execute("SELECT pg_notify('shine-test-channel-a', 'a2')", &[])
                .await
                .unwrap();
            publisher
                .execute("SELECT pg_notify('shine-test-channel-b', 'b2')", &[])
                .await
                .unwrap();

            timeout(Duration::from_secs(5), notify_b.notified())
                .await
                .expect("timed out waiting for second channel b message");

            // Give a2 a moment to have been (not) delivered.
            sleep(Duration::from_millis(200)).await;

            assert_eq!(*received_a.lock().await, vec!["a1".to_string()]);
            assert_eq!(*received_b.lock().await, vec!["b1".to_string(), "b2".to_string()]);
        }

        _ => log::warn!("Skipping test_pg_listener_shared_connection_multi_channel"),
    }
}

#[test(serial = "pg-listener")]
async fn test_pg_listener_reconnect_signal() {
    let _ = ring::default_provider().install_default();

    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            let pool = create_postgres_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();
            let helper = pool.get().await.unwrap();

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

            let listener_pid = conn
                .listener_backend_pid()
                .await
                .expect("listener connection not found");

            // kill the listener backend to force a reconnect
            helper
                .execute("SELECT pg_terminate_backend($1)", &[&listener_pid])
                .await
                .unwrap();

            timeout(Duration::from_secs(10), notify.notified())
                .await
                .expect("timed out waiting for reconnect signal");

            assert!(
                events.lock().await.contains(&None),
                "expected a None reconnect signal after backend termination"
            );

            // verify the listener recovered and still delivers notifications
            let publisher = pool.get().await.unwrap();
            publisher
                .execute("SELECT pg_notify('shine-test-reconnect', 'after-reconnect')", &[])
                .await
                .unwrap();

            timeout(Duration::from_secs(5), notify.notified())
                .await
                .expect("timed out waiting for post-reconnect notification");

            assert!(
                events.lock().await.contains(&Some("after-reconnect".to_string())),
                "expected notification after reconnect"
            );
        }

        _ => log::warn!("Skipping test_pg_listener_reconnect_signal"),
    }
}

// Regression test for the listener connection/task leak: once the last handle to the pool is
// dropped, the dedicated LISTEN backend connection must be torn down (not left open for the
// process lifetime).
#[test(serial = "pg-listener")]
async fn test_pg_listener_closes_connection_on_drop() {
    let _ = ring::default_provider().install_default();

    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            // A separate, independent pool used only to observe pg_stat_activity. It must outlive
            // the pool under test.
            let observer_pool = create_postgres_pool(&cns).await.unwrap();
            let observer = observer_pool.get().await.unwrap();

            let listener_pid = {
                let pool = create_postgres_pool(&cns).await.unwrap();
                let conn = pool.get().await.unwrap();

                conn.listen("shine-test-drop", move |_payload| {}).await.unwrap();

                let pid = conn
                    .listener_backend_pid()
                    .await
                    .expect("listener connection not found");

                // Backend must actually exist while the pool is alive.
                let alive: i64 = observer
                    .query_one("SELECT count(*) FROM pg_stat_activity WHERE pid = $1", &[&pid])
                    .await
                    .unwrap()
                    .get(0);
                assert_eq!(alive, 1, "listener backend should be alive while the pool is held");

                // Drop the checked-out connection then the pool. This releases the last PGListener
                // handle, whose Drop stops the keep-alive loop and tears down the LISTEN connection.
                drop(conn);
                drop(pool);
                pid
            };

            // Poll until the backend disappears (teardown is cooperative/async, not synchronous).
            let mut gone = false;
            for _ in 0..50 {
                let count: i64 = observer
                    .query_one("SELECT count(*) FROM pg_stat_activity WHERE pid = $1", &[&listener_pid])
                    .await
                    .unwrap()
                    .get(0);
                if count == 0 {
                    gone = true;
                    break;
                }
                sleep(Duration::from_millis(100)).await;
            }

            assert!(
                gone,
                "listener backend {listener_pid} was not closed after the pool was dropped"
            );
        }

        _ => log::warn!("Skipping test_pg_listener_closes_connection_on_drop"),
    }
}

// Regression test for the duplicate-streaming-thread path: after a reconnect there must be exactly
// one streaming thread, so a single NOTIFY is dispatched exactly once (a duplicated thread would
// deliver it twice).
#[test(serial = "pg-listener")]
async fn test_pg_listener_no_duplicate_dispatch_after_reconnect() {
    let _ = ring::default_provider().install_default();

    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            let pool = create_postgres_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();
            let helper = pool.get().await.unwrap();

            let payloads: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let payloads_write = payloads.clone();
            let notify = Arc::new(Notify::new());
            let notify_signal = notify.clone();

            conn.listen("shine-test-dup", move |payload| {
                let payloads_write = payloads_write.clone();
                let notify_signal = notify_signal.clone();
                if let Some(payload) = payload {
                    let payload = payload.to_string();
                    tokio::spawn(async move {
                        payloads_write.lock().await.push(payload);
                        notify_signal.notify_one();
                    });
                }
            })
            .await
            .unwrap();

            let listener_pid = conn
                .listener_backend_pid()
                .await
                .expect("listener connection not found");

            // Force a reconnect by killing the listener backend.
            helper
                .execute("SELECT pg_terminate_backend($1)", &[&listener_pid])
                .await
                .unwrap();

            // Wait until the listener has actually reconnected onto a new backend before publishing,
            // so the NOTIFY can't race the reconnect window.
            let mut reconnected_pid = None;
            for _ in 0..100 {
                if let Some(pid) = conn.listener_backend_pid().await {
                    if pid != listener_pid {
                        reconnected_pid = Some(pid);
                        break;
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
            let reconnected_pid = reconnected_pid.expect("listener did not reconnect onto a new backend");

            // The reconnected backend must stay stable: an orphaned streaming thread would run its
            // cleanup, clobber this fresh connection and trigger a spurious extra reconnect, moving
            // the PID again.
            sleep(Duration::from_millis(500)).await;
            assert_eq!(
                conn.listener_backend_pid().await,
                Some(reconnected_pid),
                "listener backend changed after reconnect (spurious reconnect from an orphaned streaming thread)"
            );

            let publisher = pool.get().await.unwrap();
            publisher
                .execute("SELECT pg_notify('shine-test-dup', 'once')", &[])
                .await
                .unwrap();

            timeout(Duration::from_secs(5), notify.notified())
                .await
                .expect("timed out waiting for post-reconnect notification");

            // Give any duplicate streaming thread a chance to deliver a second copy.
            sleep(Duration::from_millis(500)).await;

            let delivered = payloads.lock().await.clone();
            assert_eq!(
                delivered,
                vec!["once".to_string()],
                "notification must be delivered exactly once after reconnect (duplicates indicate an orphaned streaming thread)"
            );
        }

        _ => log::warn!("Skipping test_pg_listener_no_duplicate_dispatch_after_reconnect"),
    }
}

// Regression test: when a `listen()` call is the one that re-establishes the connection after an
// outage, it must re-subscribe every already-registered channel (via relisten), not only the newly
// added one. The bug left previously-registered channels silently unsubscribed.
#[test(serial = "pg-listener")]
async fn test_pg_listener_relisten_all_channels_on_listen_reconnect() {
    let _ = ring::default_provider().install_default();

    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            let pool = create_postgres_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();
            let helper = pool.get().await.unwrap();

            let events_a: Arc<Mutex<Vec<Option<String>>>> = Arc::new(Mutex::new(Vec::new()));
            let events_a_write = events_a.clone();
            let notify_a = Arc::new(Notify::new());
            let notify_a_signal = notify_a.clone();

            // Register channel "a" on the live connection.
            conn.listen("shine-test-relisten-a", move |payload| {
                let events_a_write = events_a_write.clone();
                let notify_a_signal = notify_a_signal.clone();
                let payload = payload.map(|s| s.to_string());
                tokio::spawn(async move {
                    events_a_write.lock().await.push(payload);
                    notify_a_signal.notify_one();
                });
            })
            .await
            .unwrap();

            let listener_pid = conn
                .listener_backend_pid()
                .await
                .expect("listener connection not found");

            // Kill the listener backend to force a reconnect.
            helper
                .execute("SELECT pg_terminate_backend($1)", &[&listener_pid])
                .await
                .unwrap();

            let events_b: Arc<Mutex<Vec<Option<String>>>> = Arc::new(Mutex::new(Vec::new()));
            let notify_b = Arc::new(Notify::new());

            // Hammer listen("b") right after the kill so a listen() call (rather than the keep-alive
            // task) drives the reconnect. Retry through the transient dead-connection window until it
            // succeeds on a fresh connection.
            let mut listened = false;
            for _ in 0..100 {
                let events_b_write = events_b.clone();
                let notify_b_signal = notify_b.clone();
                let res = conn
                    .listen("shine-test-relisten-b", move |payload| {
                        let events_b_write = events_b_write.clone();
                        let notify_b_signal = notify_b_signal.clone();
                        let payload = payload.map(|s| s.to_string());
                        tokio::spawn(async move {
                            events_b_write.lock().await.push(payload);
                            notify_b_signal.notify_one();
                        });
                    })
                    .await;
                if res.is_ok() {
                    listened = true;
                    break;
                }
                sleep(Duration::from_millis(50)).await;
            }
            assert!(listened, "listen() never succeeded after the backend was terminated");

            // Wait until the listener is on a fresh backend before publishing, so the NOTIFYs can't
            // race the reconnect window.
            let mut reconnected = false;
            for _ in 0..100 {
                if let Some(pid) = conn.listener_backend_pid().await {
                    if pid != listener_pid {
                        reconnected = true;
                        break;
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
            assert!(reconnected, "listener did not reconnect onto a new backend");

            let publisher = pool.get().await.unwrap();
            publisher
                .execute("SELECT pg_notify('shine-test-relisten-a', 'a-after')", &[])
                .await
                .unwrap();
            publisher
                .execute("SELECT pg_notify('shine-test-relisten-b', 'b-after')", &[])
                .await
                .unwrap();

            // Reconnect fires handler(None) on every channel to signal a resync, so a plain
            // notify wait could be satisfied by that None before the real payload lands. Poll the
            // recorded events for the specific payloads instead, ignoring the None signals.
            let wait_for = |events: Arc<Mutex<Vec<Option<String>>>>, notify: Arc<Notify>, want: &'static str| async move {
                let want = Some(want.to_string());
                for _ in 0..50 {
                    if events.lock().await.contains(&want) {
                        return true;
                    }
                    let _ = timeout(Duration::from_millis(100), notify.notified()).await;
                }
                events.lock().await.contains(&want)
            };

            // The pre-existing channel "a" must still deliver after the listen()-driven reconnect;
            // this is the exact property the bug broke (only "b" would have been re-LISTENed).
            assert!(
                wait_for(events_a.clone(), notify_a.clone(), "a-after").await,
                "previously-registered channel a was dropped on a listen()-triggered reconnect"
            );
            assert!(
                wait_for(events_b.clone(), notify_b.clone(), "b-after").await,
                "newly-registered channel b did not receive after reconnect"
            );
        }

        _ => log::warn!("Skipping test_pg_listener_relisten_all_channels_on_listen_reconnect"),
    }
}

// Regression test for the channel-name key mismatch: a channel whose name contains a double quote
// must still dispatch. The map must be keyed on the name PostgreSQL reports (msg.channel()), not on
// the quote-escaped SQL form, otherwise the handler never matches.
#[test(serial = "pg-listener")]
async fn test_pg_listener_channel_name_with_quote() {
    let _ = ring::default_provider().install_default();

    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            let pool = create_postgres_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();

            // Name contains a double quote; PG registers it verbatim (single quote in the name).
            let channel = r#"shine-test-"quoted"#;

            let received = Arc::new(Notify::new());
            let received_signal = received.clone();
            let received_payload = Arc::new(Mutex::new(None));
            let received_payload_write = received_payload.clone();

            conn.listen(channel, move |payload| {
                let received_payload_write = received_payload_write.clone();
                let received_signal = received_signal.clone();
                let payload = payload.map(|s| s.to_string());
                tokio::spawn(async move {
                    *received_payload_write.lock().await = payload;
                    received_signal.notify_one();
                });
            })
            .await
            .unwrap();

            // pg_notify takes the raw (unescaped) channel name as a text argument.
            let publisher = pool.get().await.unwrap();
            publisher
                .execute("SELECT pg_notify($1, 'quoted-hello')", &[&channel])
                .await
                .unwrap();

            timeout(Duration::from_secs(5), received.notified())
                .await
                .expect("timed out waiting for notification on a quoted channel name");

            assert_eq!(*received_payload.lock().await, Some("quoted-hello".to_string()));
        }

        _ => log::warn!("Skipping test_pg_listener_channel_name_with_quote"),
    }
}

// Many listen() calls fired concurrently on a fresh pool all race to open the initial connection.
// The write lock in listen() must let exactly one win and open a single shared connection; every
// channel must still be LISTENed on it and receive. Proves parallel connect is safe (no split-brain
// second connection, no lost subscriptions).
#[test(serial = "pg-listener")]
async fn test_pg_listener_parallel_connect() {
    let _ = ring::default_provider().install_default();

    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            let pool = create_postgres_pool(&cns).await.unwrap();

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

            let publisher = pool.get().await.unwrap();
            for (channel, expected) in &channels {
                publisher
                    .execute("SELECT pg_notify($1, $2)", &[channel, expected])
                    .await
                    .unwrap();
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

        _ => log::warn!("Skipping test_pg_listener_parallel_connect"),
    }
}

// listen() after close() must be rejected instead of resurrecting an unmanaged connection whose
// streaming thread would never self-heal. The guard returns before any I/O, so no server is needed.
#[test(serial = "pg-listener")]
async fn test_pg_listener_listen_after_close_is_rejected() {
    let _ = ring::default_provider().install_default();

    let config = PgConfig::from_str("postgres://user:pass@127.0.0.1:5432/db").unwrap();
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth();
    let listener = PgListener::new(
        config,
        MakeRustlsConnect::new(tls_config),
        Duration::from_secs(5),
        Duration::from_millis(500),
    );
    listener.close().await;

    let err = listener.listen("shine-test-after-close", |_| {}).await.unwrap_err();
    assert!(
        matches!(err, PgError::ListenerClosed),
        "expected PGError::ListenerClosed, got {err:?}"
    );
}

// listen() against an unreachable server must fail within the connect timeout rather than blocking
// on the socket/handshake, and close() must not be held up behind it. Uses a black-hole address
// (TEST-NET-1, RFC 5737) so the connect never completes; no server needed.
#[test(serial = "pg-listener")]
async fn test_pg_listener_listen_times_out_when_unreachable() {
    let _ = ring::default_provider().install_default();

    let config = PgConfig::from_str("postgres://user:pass@192.0.2.1:5432/db").unwrap();
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth();
    let connect_timeout = Duration::from_millis(500);
    let listener = PgListener::new(
        config,
        MakeRustlsConnect::new(tls_config),
        connect_timeout,
        Duration::from_millis(500),
    );

    let started = std::time::Instant::now();
    let err = timeout(
        Duration::from_secs(3),
        listener.listen("shine-test-unreachable", |_| {}),
    )
    .await
    .expect("listen() hung past the connect timeout")
    .unwrap_err();
    assert!(
        matches!(err, PgError::ListenerConnectTimeout | PgError::PgRawError(_)),
        "expected a connect timeout error, got {err:?}"
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
#[test(serial = "pg-listener")]
async fn test_pg_listener_duplicate_listen_is_rejected() {
    let _ = ring::default_provider().install_default();

    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            let pool = create_postgres_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();

            conn.listen("shine-test-duplicate", |_| {}).await.unwrap();
            let err = conn.listen("shine-test-duplicate", |_| {}).await.unwrap_err();
            assert!(
                matches!(err, PgError::AlreadyListening(ref channel) if channel == "shine-test-duplicate"),
                "expected PGError::AlreadyListening, got {err:?}"
            );
        }

        _ => log::warn!("Skipping test_pg_listener_duplicate_listen_is_rejected"),
    }
}
