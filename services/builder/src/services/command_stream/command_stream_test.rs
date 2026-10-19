//! The service on real Redis and Postgres: ingest, open, hydration and the orphan sweep.

use crate::{
    models::CommandStreamError,
    services::command_stream::{
        test_utils::{add, postgres_stream, redis_versions, setup, stored, unique_id, Nobody, ReadOnly},
        AllowAll, IngestOutcome, OpenError,
    },
};
use shine_infra::db::command_log::AppendOutcome;
use shine_test::test;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

#[test]
async fn open_creates_the_stream_with_the_kind_and_rejects_another_kind() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let user = Uuid::new_v4();
    let stream_id = unique_id();

    let opened = test.service.open(user, &stream_id, "chat").await.unwrap();
    assert_eq!(opened.kind.kind(), "chat");
    assert!(opened.can_write);
    assert_eq!(test.log.head(&stream_id).await.unwrap().unwrap().kind, "chat");
    assert!(
        test.redis.meta(&stream_id).await.unwrap().is_some(),
        "opening hydrates Redis"
    );

    assert!(
        test.service.open(user, &stream_id, "chat").await.is_ok(),
        "opening again is fine"
    );
    assert!(matches!(
        test.service.open(user, &stream_id, "counter").await,
        Err(OpenError::KindMismatch)
    ));
}

#[test]
async fn an_unknown_kind_is_refused() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let stream_id = unique_id();
    assert!(matches!(
        test.service.open(Uuid::new_v4(), &stream_id, "nonsense").await,
        Err(OpenError::UnknownKind)
    ));
    assert!(
        test.log.head(&stream_id).await.unwrap().is_none(),
        "nothing was created"
    );
}

#[test]
async fn access_decides_who_reads_and_writes() {
    let Some(read_only) = setup(Arc::new(ReadOnly)).await else {
        return;
    };
    let opened = read_only
        .service
        .open(Uuid::new_v4(), &unique_id(), "chat")
        .await
        .unwrap();
    assert!(!opened.can_write);

    let Some(nobody) = setup(Arc::new(Nobody)).await else {
        return;
    };
    let stream_id = unique_id();
    assert!(matches!(
        nobody.service.open(Uuid::new_v4(), &stream_id, "chat").await,
        Err(OpenError::Forbidden)
    ));
    assert!(
        nobody.log.head(&stream_id).await.unwrap().is_none(),
        "nothing was created"
    );
}

#[test]
async fn ingest_dedupes_command_id() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let stream_id = unique_id();
    let opened = test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();
    let command_id = Uuid::new_v4();

    // Same command id, same text: the second is a duplicate at the same version.
    let text = add("hi");
    let first = test
        .service
        .ingest(opened.kind.as_ref(), &stream_id, command_id, Uuid::nil(), &text)
        .await
        .unwrap();
    let second = test
        .service
        .ingest(opened.kind.as_ref(), &stream_id, command_id, Uuid::nil(), &text)
        .await
        .unwrap();
    assert_eq!(
        (first, second),
        (IngestOutcome::Accepted(1), IngestOutcome::Duplicate(1))
    );
    assert_eq!(redis_versions(&test, &stream_id).await, vec![1]);
}

#[test]
async fn ingest_rejects_invalid_and_oversized_commands() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let stream_id = unique_id();
    let opened = test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();

    let oversized = add(&"x".repeat(300));
    let invalid = [
        "not json".to_string(),
        r#"{"type":"edit"}"#.to_string(),
        r#"{"type":"add"}"#.to_string(),
        add("   "),
        oversized,
    ];
    for command in invalid {
        let outcome = test
            .service
            .ingest(opened.kind.as_ref(), &stream_id, Uuid::new_v4(), Uuid::nil(), &command)
            .await
            .unwrap();
        assert_eq!(outcome, IngestOutcome::Invalid, "{command:.40}");
    }
    assert!(
        redis_versions(&test, &stream_id).await.is_empty(),
        "nothing invalid was stored"
    );
}

#[test]
async fn ingest_canonicalizes_the_command_text() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let stream_id = unique_id();
    let opened = test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();

    test.service
        .ingest(
            opened.kind.as_ref(),
            &stream_id,
            Uuid::new_v4(),
            Uuid::nil(),
            r#"{ "text": "hi", "type": "add", "extra": 1 }"#,
        )
        .await
        .unwrap();
    let (_, entries) = test.redis.read_after(&stream_id, 0, 10).await.unwrap();
    assert_eq!(entries[0].data, r#"{"type":"add","text":"hi"}"#);
}

#[test]
async fn ensure_hydrated_copies_the_tail_from_the_snapshot_horizon() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    // Snapshots at 5 and 10 with 2 kept: the horizon is 5.
    let stream_id = postgres_stream(&test, 12, &[5, 10]).await;
    assert!(test.redis.meta(&stream_id).await.unwrap().is_none());

    test.service.ensure_hydrated(&stream_id).await.unwrap();

    let meta = test.redis.meta(&stream_id).await.unwrap().unwrap();
    assert_eq!((meta.version, meta.persisted), (12, 12));
    assert_eq!(
        meta.generation,
        test.log.head(&stream_id).await.unwrap().unwrap().generation
    );
    assert_eq!(redis_versions(&test, &stream_id).await, (5..=12).collect::<Vec<_>>());
}

#[test]
async fn ensure_hydrated_without_enough_snapshots_copies_the_whole_short_log() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let stream_id = postgres_stream(&test, 3, &[]).await;
    test.service.ensure_hydrated(&stream_id).await.unwrap();
    assert_eq!(redis_versions(&test, &stream_id).await, vec![1, 2, 3]);

    // One snapshot while two are kept: still no horizon.
    let one = postgres_stream(&test, 8, &[5]).await;
    test.service.ensure_hydrated(&one).await.unwrap();
    assert_eq!(redis_versions(&test, &one).await, (1..=8).collect::<Vec<_>>());
}

#[test]
async fn ensure_hydrated_of_an_unknown_stream_is_not_found() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    assert!(matches!(
        test.service.ensure_hydrated(&unique_id()).await,
        Err(CommandStreamError::StreamNotFound)
    ));
}

#[test]
async fn ensure_hydrated_resets_a_redis_copy_behind_postgres() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let stream_id = unique_id();
    test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();
    let stale = test.redis.meta(&stream_id).await.unwrap().unwrap();

    // Postgres moves on to a newer generation with commands the Redis copy never saw.
    let head = test.log.begin_generation(&stream_id).await.unwrap();
    let commands: Vec<_> = (1..=3).map(|v| stored(v, head.generation)).collect();
    assert_eq!(
        test.log.append(&stream_id, &commands).await.unwrap(),
        AppendOutcome::Appended { version: 3 }
    );

    test.service.ensure_hydrated(&stream_id).await.unwrap();
    let meta = test.redis.meta(&stream_id).await.unwrap().unwrap();
    assert_eq!(meta.version, 3, "rebuilt from Postgres");
    assert!(meta.generation > stale.generation);
}

#[test]
async fn concurrent_hydrations_converge() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let stream_id = postgres_stream(&test, 6, &[]).await;
    let generation_before = test.log.head(&stream_id).await.unwrap().unwrap().generation;

    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let (service, stream_id) = (test.service.clone(), stream_id.clone());
            tokio::spawn(async move { service.ensure_hydrated(&stream_id).await })
        })
        .collect();
    for task in tasks {
        task.await.unwrap().unwrap();
    }

    let head = test.log.head(&stream_id).await.unwrap().unwrap();
    assert_eq!(head.generation, generation_before + 1, "the stream was hydrated once");
    assert_eq!(redis_versions(&test, &stream_id).await, (1..=6).collect::<Vec<_>>());
}

#[test]
async fn a_refused_ingest_starts_a_background_hydration() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let stream_id = postgres_stream(&test, 2, &[]).await;
    let opened = test.service.kinds().get("chat").unwrap();

    let outcome = test
        .service
        .ingest(opened.as_ref(), &stream_id, Uuid::new_v4(), Uuid::nil(), &add("hi"))
        .await
        .unwrap();
    assert_eq!(outcome, IngestOutcome::NotReady);

    let hydrated = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if test.redis.meta(&stream_id).await.unwrap().is_some() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(hydrated.is_ok(), "Redis got a copy in the background");

    let retry = test
        .service
        .ingest(opened.as_ref(), &stream_id, Uuid::new_v4(), Uuid::nil(), &add("hi"))
        .await
        .unwrap();
    assert_eq!(retry, IngestOutcome::Accepted(3));
}

#[test]
async fn sweep_orphans_persists_and_retires_idle_streams() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let stream_id = unique_id();
    let opened = test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();
    for text in ["a", "b", "c"] {
        test.service
            .ingest(
                opened.kind.as_ref(),
                &stream_id,
                Uuid::new_v4(),
                Uuid::nil(),
                &add(text),
            )
            .await
            .unwrap();
    }
    assert_eq!(
        test.log.head(&stream_id).await.unwrap().unwrap().version,
        0,
        "nothing persisted yet"
    );

    test.service.sweep_orphans().await;

    assert_eq!(test.log.head(&stream_id).await.unwrap().unwrap().version, 3);
    let meta = test.redis.meta(&stream_id).await.unwrap().unwrap();
    assert_eq!(meta.persisted, 3);
    assert!(
        test.redis.idle_streams(Duration::ZERO, 10).await.unwrap().is_empty(),
        "a fully persisted stream is retired"
    );
}

#[test]
async fn sweep_orphans_drops_a_redis_copy_that_diverged_from_postgres() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let stream_id = unique_id();
    let opened = test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();
    test.service
        .ingest(
            opened.kind.as_ref(),
            &stream_id,
            Uuid::new_v4(),
            Uuid::nil(),
            &add("redis"),
        )
        .await
        .unwrap();

    // Postgres already holds another command at version 1 (a copy that went back in time).
    let generation = test.log.head(&stream_id).await.unwrap().unwrap().generation;
    test.log.append(&stream_id, &[stored(1, generation)]).await.unwrap();

    test.service.sweep_orphans().await;

    assert!(
        test.redis.meta(&stream_id).await.unwrap().is_none(),
        "Postgres wins, the Redis copy is dropped"
    );
    assert_eq!(test.log.head(&stream_id).await.unwrap().unwrap().version, 1);
}
