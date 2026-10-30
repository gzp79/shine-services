use super::*;
use crate::{
    repositories::command_log::CommandLogDb,
    services::command_stream::{CommandStreamConfig, PersistResult, PersistenceWorker},
    testing::command_stream::{create_test_inbox, create_test_log, persist_chat_logs, stream_id, ChatLog, TestEgress},
};
use shine_game::replica::{ChatState, ReplayState};
use shine_test::test;

#[test]
async fn actor_submits_and_promotes_optimistic_commands() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let Some(log) = create_test_log().await else { return };
    let stream_id = stream_id(ChatState::KIND);
    let room = ChatLog::new("optimistic", 2);
    let started = PersistenceWorker::<ChatState, _, _>::new(
        stream_id.clone(),
        inbox.db.clone(),
        log.db.clone(),
        CommandStreamConfig::default(),
    )
    .start()
    .await
    .unwrap();
    let mut actor = StreamActor::new(
        stream_id,
        inbox.db.clone(),
        TestEgress::default(),
        started.snapshot,
        started.tail,
        started.handle,
        started.results,
    );
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    actor.subscribe(first, 0);
    actor.subscribe(second, 1);
    actor.record_sent(first, 2);
    actor.append_optimistic(room.commands.clone());

    assert!(actor.persist_optimistic().await.unwrap());
    assert!(
        !actor.persist_optimistic().await.unwrap(),
        "only one job may be in flight"
    );
    let result = actor.recv_persistence().await.unwrap();
    let PersistResult::Advanced {
        generation,
        canonical,
        snapshot,
        ..
    } = result
    else {
        panic!("expected the optimistic batch to be confirmed");
    };
    assert!(actor.accept_canonical(generation, &canonical, snapshot));

    assert!(actor.optimistic().is_empty());
    assert_eq!(actor.confirmed(), room.commands);
    assert_eq!(actor.sent_through(first), Some(2));
    assert_eq!(actor.sent_through(second), Some(1));

    drop(actor);
    started.task.await.unwrap();
}

#[test]
async fn actor_adopts_a_reload_and_invalidates_optimistic_history() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let Some(log) = create_test_log().await else { return };
    let stream_id = stream_id(ChatState::KIND);
    let started = PersistenceWorker::<ChatState, _, _>::new(
        stream_id.clone(),
        inbox.db.clone(),
        log.db.clone(),
        CommandStreamConfig::default(),
    )
    .start()
    .await
    .unwrap();
    let mut actor = StreamActor::new(
        stream_id.clone(),
        inbox.db.clone(),
        TestEgress::default(),
        started.snapshot,
        started.tail,
        started.handle,
        started.results,
    );
    let optimistic = ChatLog::new("mine", 2);
    actor.append_optimistic(optimistic.commands.clone());

    let canonical = ChatLog::new("other", 2);
    persist_chat_logs(&log.db, &stream_id, &canonical, 1, 2).await.unwrap();
    assert!(actor.persist_optimistic().await.unwrap());

    let result = actor.recv_persistence().await.unwrap();
    let PersistResult::Reloaded { generation, snapshot, tail, .. } = result else {
        panic!("expected the losing optimistic history to reload");
    };
    assert!(actor.accept_reload(generation, snapshot, tail.to_vec()));

    assert_eq!(actor.generation(), 1);
    assert!(actor.optimistic().is_empty());
    assert_eq!(actor.confirmed(), canonical.commands);
    assert_eq!(
        log.db.head(&stream_id).await.unwrap().unwrap().chain_hash,
        canonical.replica.chain_hash()
    );

    drop(actor);
    started.task.await.unwrap();
}
