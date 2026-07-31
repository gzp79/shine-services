use rustls::crypto::ring;
use shine_infra::db::create_postgres_pool;
use shine_test::test;
use std::{env, sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, Notify},
    time::{sleep, timeout},
};

#[test]
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

#[test]
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

#[test]
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
#[test]
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
#[test]
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

// Regression test for the channel-name key mismatch: a channel whose name contains a double quote
// must still dispatch. The map must be keyed on the name PostgreSQL reports (msg.channel()), not on
// the quote-escaped SQL form, otherwise the handler never matches.
#[test]
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
