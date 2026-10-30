use super::*;
use crate::repositories::{
    command_inbox::tests::test_utils::create_test_inbox,
    command_log::{
        tests::{create_test_log, fetch_all_commands, seed_chat_log_and_snapshots, stream_id, ChatLog, TestLog},
        CommandLogDb,
    },
};
use shine_game::replica::ChatState;
use shine_test::test;

async fn fixture() -> Option<(
    crate::repositories::command_inbox::tests::test_utils::TestInbox,
    TestLog,
)> {
    Some((create_test_inbox(60).await?, create_test_log().await?))
}

fn worker(
    stream_id: StreamId,
    inbox: &crate::repositories::command_inbox::tests::test_utils::TestInbox,
    log: &TestLog,
    config: CommandStreamConfig,
) -> PersistenceWorker<ChatState, impl CommandInboxDb + 'static, impl CommandLogDb + 'static> {
    PersistenceWorker::new(stream_id, inbox.db.clone(), log.db.clone(), config)
}

#[test]
async fn bootstrap_loads_the_latest_snapshot_and_tail() {
    let Some((inbox, log)) = fixture().await else { return };
    let (stream_id, room) = seed_chat_log_and_snapshots(&log, 10, &[4, 7]).await;

    let started = worker(stream_id, &inbox, &log, CommandStreamConfig::default())
        .start()
        .await
        .unwrap();

    assert_eq!(started.snapshot.as_ref().map(|snapshot| snapshot.version), Some(7));
    assert_eq!(
        started.tail.iter().map(|command| command.version).collect::<Vec<_>>(),
        vec![8, 9, 10]
    );
    assert_eq!(started.tail, room.commands[7..]);
    drop(started.handle);
    drop(started.results);
    started.task.await.unwrap();
}

#[test]
async fn persistence_appends_replays_and_returns_a_snapshot() {
    let Some((inbox, log)) = fixture().await else { return };
    let stream_id = stream_id(ChatState::KIND);
    let room = ChatLog::new("persist", 4);
    let config = CommandStreamConfig {
        snapshot_every: 2,
        ..CommandStreamConfig::default()
    };
    let mut started = worker(stream_id.clone(), &inbox, &log, config).start().await.unwrap();

    started
        .handle
        .persist(PersistJob {
            generation: 3,
            baseline_version: 0,
            baseline_chain_hash: 0,
            commands: room.commands.clone().into(),
        })
        .await
        .unwrap();

    let result = started.results.recv().await.unwrap();
    let PersistResult::Advanced {
        generation,
        canonical,
        snapshot,
        checkpoint,
    } = result
    else {
        panic!("expected an advanced result");
    };
    assert_eq!(generation, 3);
    assert_eq!(&*canonical, room.commands);
    assert_eq!(snapshot.as_ref().map(|value| value.version), Some(4));
    assert_eq!(checkpoint.map(|value| value.version), Some(4));
    assert_eq!(fetch_all_commands(&log.db, &stream_id).await, room.commands);
    assert_eq!(
        log.db
            .latest_snapshot(&stream_id, 4)
            .await
            .unwrap()
            .map(|value| value.version),
        Some(4)
    );

    drop(started.handle);
    drop(started.results);
    started.task.await.unwrap();
}

#[test]
async fn concurrent_workers_converge_on_the_same_canonical_batch() {
    let Some((inbox, log)) = fixture().await else { return };
    let stream_id = stream_id(ChatState::KIND);
    let room = ChatLog::new("shared", 6);
    let mut first = worker(stream_id.clone(), &inbox, &log, CommandStreamConfig::default())
        .start()
        .await
        .unwrap();
    let mut second = worker(stream_id.clone(), &inbox, &log, CommandStreamConfig::default())
        .start()
        .await
        .unwrap();

    let job = |generation| PersistJob {
        generation,
        baseline_version: 0,
        baseline_chain_hash: 0,
        commands: room.commands.clone().into(),
    };
    let (first_sent, second_sent) = tokio::join!(first.handle.persist(job(1)), second.handle.persist(job(2)));
    first_sent.unwrap();
    second_sent.unwrap();
    let (first_result, second_result) = tokio::join!(first.results.recv(), second.results.recv());

    for result in [first_result.unwrap(), second_result.unwrap()] {
        let PersistResult::Advanced { canonical, .. } = result else {
            panic!("both instances must converge without a reload");
        };
        assert_eq!(&*canonical, room.commands);
    }
    assert_eq!(fetch_all_commands(&log.db, &stream_id).await, room.commands);

    drop(first.handle);
    drop(first.results);
    drop(second.handle);
    drop(second.results);
    first.task.await.unwrap();
    second.task.await.unwrap();
}
