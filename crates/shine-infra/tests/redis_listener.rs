use redis::AsyncCommands;
use shine_infra::db::create_redis_pool;
use shine_test::test;
use std::{env, sync::Arc, time::Duration};
use tokio::sync::Notify;

#[test]
async fn test_redis_listener_pub_sub() {
    match env::var("SHINE_TEST_REDIS_CNS") {
        Ok(cns) => {
            let pool = create_redis_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();

            let received = Arc::new(Notify::new());
            let received_signal = received.clone();
            let received_payload = Arc::new(tokio::sync::Mutex::new(None));
            let received_payload_write = received_payload.clone();

            conn.listen("shine-test-channel", move |payload| {
                let received_payload_write = received_payload_write.clone();
                let payload = payload.to_string();
                let received_signal = received_signal.clone();
                tokio::spawn(async move {
                    *received_payload_write.lock().await = Some(payload);
                    received_signal.notify_one();
                });
            })
            .await
            .unwrap();

            let mut publisher = pool.get().await.unwrap();
            let _: () = publisher.publish("shine-test-channel", "hello").await.unwrap();

            tokio::time::timeout(Duration::from_secs(5), received.notified())
                .await
                .expect("timed out waiting for the pub/sub message");

            let payload = received_payload.lock().await.clone();
            assert_eq!(payload, Some("hello".to_string()));
        }

        _ => log::warn!("Skipping test_redis_listener_pub_sub"),
    }
}

#[test]
async fn test_redis_listener_shared_connection_multi_channel() {
    match env::var("SHINE_TEST_REDIS_CNS") {
        Ok(cns) => {
            let pool = create_redis_pool(&cns).await.unwrap();
            let conn = pool.get().await.unwrap();

            let received_a = Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
            let received_a_write = received_a.clone();
            let notify_a = Arc::new(Notify::new());
            let notify_a_signal = notify_a.clone();

            let received_b = Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
            let received_b_write = received_b.clone();
            let notify_b = Arc::new(Notify::new());
            let notify_b_signal = notify_b.clone();

            conn.listen("shine-test-channel-a", move |payload| {
                let received_a_write = received_a_write.clone();
                let notify_a_signal = notify_a_signal.clone();
                let payload = payload.to_string();
                tokio::spawn(async move {
                    received_a_write.lock().await.push(payload);
                    notify_a_signal.notify_one();
                });
            })
            .await
            .unwrap();

            conn.listen("shine-test-channel-b", move |payload| {
                let received_b_write = received_b_write.clone();
                let notify_b_signal = notify_b_signal.clone();
                let payload = payload.to_string();
                tokio::spawn(async move {
                    received_b_write.lock().await.push(payload);
                    notify_b_signal.notify_one();
                });
            })
            .await
            .unwrap();

            // Both channels are served by the same shared connection; both must still receive.
            let mut publisher = pool.get().await.unwrap();
            let _: () = publisher.publish("shine-test-channel-a", "a1").await.unwrap();
            let _: () = publisher.publish("shine-test-channel-b", "b1").await.unwrap();

            tokio::time::timeout(Duration::from_secs(5), notify_a.notified())
                .await
                .expect("timed out waiting for channel a message");
            tokio::time::timeout(Duration::from_secs(5), notify_b.notified())
                .await
                .expect("timed out waiting for channel b message");

            assert_eq!(*received_a.lock().await, vec!["a1".to_string()]);
            assert_eq!(*received_b.lock().await, vec!["b1".to_string()]);

            // Unlisten channel a; channel b must keep receiving on the same shared connection,
            // and channel a must no longer be dispatched to.
            conn.unlisten("shine-test-channel-a").await.unwrap();

            let _: () = publisher.publish("shine-test-channel-a", "a2").await.unwrap();
            let _: () = publisher.publish("shine-test-channel-b", "b2").await.unwrap();

            tokio::time::timeout(Duration::from_secs(5), notify_b.notified())
                .await
                .expect("timed out waiting for second channel b message");

            // Give a2 a moment to have been (not) delivered.
            tokio::time::sleep(Duration::from_millis(200)).await;

            assert_eq!(*received_a.lock().await, vec!["a1".to_string()]);
            assert_eq!(*received_b.lock().await, vec!["b1".to_string(), "b2".to_string()]);
        }

        _ => log::warn!("Skipping test_redis_listener_shared_connection_multi_channel"),
    }
}
