//! Unit tests of the bounded session lookups of the checker.

use super::*;
use ring::rand::SystemRandom;
use shine_test::test;
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

fn connections(count: usize) -> Vec<(Uuid, Uuid, SessionKey)> {
    let random = SystemRandom::new();
    (0..count)
        .map(|_| (Uuid::new_v4(), Uuid::new_v4(), SessionKey::new_random(&random).unwrap()))
        .collect()
}

#[test]
async fn lookups_overlap_up_to_the_concurrency_and_not_beyond() {
    let (in_flight, peak) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let (count, concurrency, latency) = (40, 8, Duration::from_millis(20));

    let start = Instant::now();
    let verdicts = check_sessions(connections(count), concurrency, |_, _| async {
        let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        peak.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(latency).await;
        in_flight.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    })
    .await;

    assert_eq!(verdicts.len(), count);
    assert_eq!(peak.load(Ordering::SeqCst), concurrency);
    // One after another it would take count * latency (800 ms).
    assert!(start.elapsed() < latency * (count as u32) / 2, "{:?}", start.elapsed());
}

#[test]
async fn every_connection_gets_its_own_verdict() {
    let all = connections(10);
    let expired: Vec<Uuid> = all.iter().step_by(3).map(|(user_id, _, _)| *user_id).collect();

    let verdicts = check_sessions(all.clone(), 4, |user_id, _| {
        let expired = expired.contains(&user_id);
        async move {
            if expired {
                Err(UserSessionError::SessionExpired)
            } else {
                Ok(())
            }
        }
    })
    .await;

    assert_eq!(verdicts.len(), all.len());
    for (user_id, connection_id, verdict) in verdicts {
        let (_, expected_connection, _) = all.iter().find(|(u, _, _)| *u == user_id).unwrap();
        assert_eq!(connection_id, *expected_connection);
        assert_eq!(verdict.is_err(), expired.contains(&user_id), "{user_id}");
    }
}
