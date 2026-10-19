//! Chat delivery of the hub: every connection gets the room's comments exactly once and in stream order,
//! pushed on an append notification ahead of the periodic tick.

use crate::{
    models::messages::DisconnectReason,
    repositories::chat_comments::redis::RedisChatCommentsDb,
    services::hub_test_utils::{
        barrier, connect, create_test_hub, egress_state, Egress, EgressState, Rng, TestHub, TestHubOptions,
        BARRIER_TIMEOUT,
    },
    services::ChatService,
};
use shine_test::test;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{task::JoinHandle, time::timeout};
use uuid::Uuid;

async fn chat_writer(test: &TestHub) -> ChatService {
    let chat_db = RedisChatCommentsDb::new(&test.redis, 200, 3600).await.unwrap();
    ChatService::with_room(chat_db, test.chat_room.clone())
}

/// Collects a connection's egress into a shared state, so progress can be polled while it runs.
fn spawn_shared_reader(mut rx: Egress) -> (Arc<Mutex<EgressState>>, JoinHandle<()>) {
    let state = Arc::new(Mutex::new(EgressState::default()));
    let reader = {
        let state = state.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                state.lock().unwrap().record(message);
            }
            state.lock().unwrap().closed = true;
        })
    };
    (state, reader)
}

fn chats_of(state: &Arc<Mutex<EgressState>>) -> Vec<String> {
    state.lock().unwrap().chats.clone()
}

/// Redis stream ids `<ms>-<seq>` in numeric order.
fn stream_order(id: &str) -> (u64, u64) {
    let (ms, seq) = id.split_once('-').unwrap();
    (ms.parse().unwrap(), seq.parse().unwrap())
}

#[test(serial = "hub-service")]
async fn an_append_notification_delivers_ahead_of_the_tick() {
    // With an hour-long tick, only the pub/sub wake can deliver within the test window.
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let writer = chat_writer(&test).await;
    let user = Uuid::new_v4();
    let (connection, mut rx) = connect(&test.hub, user, 8);
    barrier(&test.hub, &mut test.events).await;

    let author = Uuid::new_v4();
    let id = writer.append_comment(author, "woken-up").await.unwrap();
    let message = timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("the append notification must wake the dispatcher");
    let Some(crate::models::messages::HubMessage::Chat(batch)) = message else {
        panic!("expected a chat batch, got {message:?}");
    };
    assert_eq!(batch.comments.len(), 1);
    let comment = &batch.comments[0];
    assert_eq!(
        (comment.id.as_str(), comment.user_id, comment.text.as_str()),
        (id.as_str(), author, "woken-up")
    );

    test.hub.request_disconnection(user, connection).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn ticks_and_spurious_wakes_deliver_nothing_new() {
    let Some(mut test) = create_test_hub(TestHubOptions {
        chat: Duration::from_millis(50),
        ..Default::default()
    })
    .await
    else {
        return;
    };
    let writer = chat_writer(&test).await;
    let user = Uuid::new_v4();
    let id = writer.append_comment(Uuid::new_v4(), "history").await.unwrap();
    let (connection, mut rx) = connect(&test.hub, user, 8);
    barrier(&test.hub, &mut test.events).await;

    // The retained history arrives once; many ticks and a PUBLISH without an append add nothing.
    tokio::time::sleep(Duration::from_millis(200)).await;
    {
        let mut conn = test.redis.get().await.unwrap();
        let _: () = redis::cmd("PUBLISH")
            .arg(format!("chat:{}", test.chat_room))
            .arg("0-0")
            .query_async(&mut *conn)
            .await
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(egress_state(&mut rx).chats, [id]);

    test.hub.request_disconnection(user, connection).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn concurrent_writers_reach_every_connection_once_and_in_order() {
    let Some(mut test) = create_test_hub(TestHubOptions {
        chat: Duration::from_millis(50),
        ..Default::default()
    })
    .await
    else {
        return;
    };
    const WRITERS: usize = 4;
    // More than a tick's batch limit (128) in total, within the retained history (200).
    const COMMENTS_PER_WRITER: usize = 45;
    const TOTAL: usize = WRITERS * COMMENTS_PER_WRITER;

    let steady: Vec<_> = (0..6)
        .map(|_| {
            let user = Uuid::new_v4();
            let (connection, rx) = connect(&test.hub, user, 1024);
            (user, connection, spawn_shared_reader(rx))
        })
        .collect();
    let leaver_user = Uuid::new_v4();
    let (leaver, leaver_rx) = connect(&test.hub, leaver_user, 1024);
    let (leaver_state, _) = spawn_shared_reader(leaver_rx);
    let reconnect_user = Uuid::new_v4();
    let (_replaced, replaced_rx) = connect(&test.hub, reconnect_user, 1024);
    let (replaced_state, _) = spawn_shared_reader(replaced_rx);
    barrier(&test.hub, &mut test.events).await;

    let writers: Vec<JoinHandle<Vec<String>>> = (0..WRITERS)
        .map(|seed| {
            let writer = tokio::spawn({
                let redis = test.redis.clone();
                let room = test.chat_room.clone();
                async move {
                    let chat_db = RedisChatCommentsDb::new(&redis, 200, 3600).await.unwrap();
                    ChatService::with_room(chat_db, room)
                }
            });
            tokio::spawn(async move {
                let writer = writer.await.unwrap();
                let mut rng = Rng(0xCAFE + seed as u64);
                let mut ids = vec![];
                for idx in 0..COMMENTS_PER_WRITER {
                    ids.push(
                        writer
                            .append_comment(Uuid::new_v4(), &format!("{seed}-{idx}"))
                            .await
                            .unwrap(),
                    );
                    tokio::time::sleep(Duration::from_millis(rng.below(3))).await;
                }
                ids
            })
        })
        .collect();

    // Mid-way, one client leaves and another reconnects, which replays the history from the start.
    tokio::time::sleep(Duration::from_millis(60)).await;
    test.hub.request_disconnection(leaver_user, leaver).unwrap();
    let (reconnected, reconnected_rx) = connect(&test.hub, reconnect_user, 1024);
    let (reconnected_state, _) = spawn_shared_reader(reconnected_rx);

    let mut expected = vec![];
    for writer in writers {
        expected.extend(writer.await.unwrap());
    }
    expected.sort_by_key(|id| stream_order(id));
    assert_eq!(expected.len(), TOTAL);
    // A connection joining after the writes gets the whole retained history.
    let late_user = Uuid::new_v4();
    let (late, late_rx) = connect(&test.hub, late_user, 1024);
    let (late_state, _) = spawn_shared_reader(late_rx);

    let full: Vec<_> = steady
        .iter()
        .map(|(_, _, (state, _))| state)
        .chain([&reconnected_state, &late_state])
        .collect();
    timeout(BARRIER_TIMEOUT, async {
        while full.iter().any(|state| chats_of(state).len() < TOTAL) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("every connection must receive every comment");
    // Let further ticks run: nothing may be delivered twice.
    tokio::time::sleep(Duration::from_millis(200)).await;

    for state in &full {
        assert_eq!(chats_of(state), expected, "exactly once, in stream order");
    }
    for (name, state) in [("leaver", &leaver_state), ("replaced", &replaced_state)] {
        let chats = chats_of(state);
        assert_eq!(chats, expected[..chats.len()], "the {name} got a prefix of the stream");
    }
    assert_eq!(replaced_state.lock().unwrap().notices, [DisconnectReason::Replaced]);

    for (user, connection, _) in &steady {
        test.hub.request_disconnection(*user, *connection).unwrap();
    }
    test.hub.request_disconnection(reconnect_user, reconnected).unwrap();
    test.hub.request_disconnection(late_user, late).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

async fn xadd(test: &TestHub, id: &str, fields: &[(&str, &str)]) {
    let mut conn = test.redis.get().await.unwrap();
    let mut cmd = redis::cmd("XADD");
    cmd.arg(format!("chat:{}", test.chat_room)).arg(id);
    for (name, value) in fields {
        cmd.arg(*name).arg(*value);
    }
    let _: String = cmd.query_async(&mut *conn).await.unwrap();
}

async fn wait_for_chats(state: &Arc<Mutex<EgressState>>, count: usize) {
    timeout(BARRIER_TIMEOUT, async {
        while chats_of(state).len() < count {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("expected {count} comments, got {}", chats_of(state).len()));
}

#[test(serial = "hub-service")]
async fn a_backlog_beyond_one_page_is_delivered_in_a_single_tick() {
    // Only the append wakes the dispatcher: the tick is an hour away.
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let writer = chat_writer(&test).await;
    for i in 0..150 {
        writer.append_comment(Uuid::new_v4(), &format!("old-{i}")).await.unwrap();
    }
    let user = Uuid::new_v4();
    let (connection, rx) = connect(&test.hub, user, 1024);
    let (state, _) = spawn_shared_reader(rx);
    barrier(&test.hub, &mut test.events).await;

    // One wake, more than the 128 entries of a page waiting.
    writer.append_comment(Uuid::new_v4(), "wake").await.unwrap();
    wait_for_chats(&state, 151).await;

    test.hub.request_disconnection(user, connection).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn a_joining_connection_replaying_history_does_not_hold_the_others_back() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let writer = chat_writer(&test).await;
    for i in 0..190 {
        writer.append_comment(Uuid::new_v4(), &format!("old-{i}")).await.unwrap();
    }
    let (steady_user, joiner_user) = (Uuid::new_v4(), Uuid::new_v4());
    let (steady, steady_rx) = connect(&test.hub, steady_user, 1024);
    let (steady_state, _) = spawn_shared_reader(steady_rx);
    barrier(&test.hub, &mut test.events).await;
    writer.append_comment(Uuid::new_v4(), "wake-1").await.unwrap();
    wait_for_chats(&steady_state, 191).await;

    // A newcomer starts from the beginning of the retained history; the live comment must still
    // reach the connection that is current right away.
    let (joiner, joiner_rx) = connect(&test.hub, joiner_user, 1024);
    let (joiner_state, _) = spawn_shared_reader(joiner_rx);
    barrier(&test.hub, &mut test.events).await;
    let live = writer.append_comment(Uuid::new_v4(), "live").await.unwrap();
    wait_for_chats(&steady_state, 192).await;
    assert_eq!(chats_of(&steady_state).last(), Some(&live));

    wait_for_chats(&joiner_state, 192).await;
    assert_eq!(chats_of(&joiner_state), chats_of(&steady_state), "the newcomer catches up in order");

    test.hub.request_disconnection(steady_user, steady).unwrap();
    test.hub.request_disconnection(joiner_user, joiner).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn unreadable_entries_are_skipped_instead_of_blocking_the_stream() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let writer = chat_writer(&test).await;
    // A whole page of entries without the fields of a comment.
    for i in 1..=130 {
        xadd(&test, &format!("{i}-0"), &[("junk", "x")]).await;
    }
    let user = Uuid::new_v4();
    let (connection, rx) = connect(&test.hub, user, 1024);
    let (state, _) = spawn_shared_reader(rx);
    barrier(&test.hub, &mut test.events).await;

    let id = writer.append_comment(Uuid::new_v4(), "after the junk").await.unwrap();
    wait_for_chats(&state, 1).await;
    assert_eq!(chats_of(&state), [id]);

    test.hub.request_disconnection(user, connection).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn a_stream_restarting_behind_the_cursors_is_replayed() {
    let tick = Duration::from_millis(20);
    let Some(mut test) = create_test_hub(TestHubOptions { chat: tick, ..Default::default() }).await else {
        return;
    };
    let writer = chat_writer(&test).await;
    // The stream is at a far-future id, as a stream that outlived a clock far ahead of the new one.
    xadd(&test, "99999999999999-0", &[("user", &Uuid::new_v4().to_string()), ("text", "future")]).await;
    let user = Uuid::new_v4();
    let (connection, rx) = connect(&test.hub, user, 1024);
    let (state, _) = spawn_shared_reader(rx);
    barrier(&test.hub, &mut test.events).await;
    wait_for_chats(&state, 1).await;

    // The stream is lost and restarts with ordinary ids, all below the connection's cursor.
    {
        let mut conn = test.redis.get().await.unwrap();
        let _: () = redis::cmd("DEL")
            .arg(format!("chat:{}", test.chat_room))
            .query_async(&mut *conn)
            .await
            .unwrap();
    }
    let id = writer.append_comment(Uuid::new_v4(), "restarted").await.unwrap();
    wait_for_chats(&state, 2).await;
    assert_eq!(chats_of(&state).last(), Some(&id));

    test.hub.request_disconnection(user, connection).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn an_append_stores_trims_and_expires_the_stream_in_one_step() {
    let Some(test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let writer = chat_writer(&test).await;
    for i in 0..450 {
        writer.append_comment(Uuid::new_v4(), &format!("m{i}")).await.unwrap();
    }

    let mut conn = test.redis.get().await.unwrap();
    let key = format!("chat:{}", test.chat_room);
    let len: usize = redis::cmd("XLEN").arg(&key).query_async(&mut *conn).await.unwrap();
    let ttl: i64 = redis::cmd("TTL").arg(&key).query_async(&mut *conn).await.unwrap();
    // The limit is approximate (whole macro nodes are trimmed), but the stream cannot keep growing.
    assert!((200..450).contains(&len), "the stream must be trimmed near its limit of 200, got {len}");
    assert!(ttl > 0 && ttl <= 3600, "the stream must expire, got ttl {ttl}");
    assert!(writer.last_id().await.unwrap().is_some());
}
