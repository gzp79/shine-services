//! The snapshot route's logic on real stores; the session check is the extractor's and is covered by the API tests.

use super::{problem_of, snapshot_frame};
use crate::{
    models::CommandStreamError,
    services::command_stream::{
        test_utils::{add, postgres_stream, setup, unique_id},
        AllowAll,
    },
};
use axum::http::StatusCode;
use shine_game::replay::protocol::{decode_snapshot_data, hash_from_hex, ServerFrame};
use shine_infra::db::redis::RedisError;
use shine_test::test;
use std::sync::Arc;
use uuid::Uuid;

#[test]
async fn the_snapshot_route_returns_the_snapshot_plus_the_tail_in_wire_form() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let stream_id = postgres_stream(&test, 12, &[5, 10]).await;

    let frame = snapshot_frame(Some(&test.service), Uuid::nil(), stream_id.as_str())
        .await
        .unwrap();
    let ServerFrame::StreamSync {
        stream_id: id,
        snapshot_every,
        snapshot,
        commands,
        ..
    } = frame
    else {
        panic!("expected a streamSync");
    };
    assert_eq!((id.as_str(), snapshot_every), (stream_id.as_str(), 5));
    let snapshot = snapshot.unwrap();
    assert_eq!(snapshot.version, 10);
    assert_eq!(
        decode_snapshot_data(&snapshot.data),
        Some(b"s".to_vec()),
        "the stored snapshot, as base64"
    );
    assert!(hash_from_hex(&snapshot.hash).is_some());
    assert_eq!(commands.iter().map(|c| c.version).collect::<Vec<_>>(), [11, 12]);
    assert_eq!(commands[0].command, add("m11"), "the command text is sent as stored");
}

#[test]
async fn the_snapshot_route_answers_with_the_status_of_the_failure() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let status = |problem: crate::routes::command_stream::Problem| problem.status;

    let unknown = snapshot_frame(Some(&test.service), Uuid::nil(), unique_id().as_str()).await;
    assert_eq!(status(unknown.unwrap_err()), StatusCode::NOT_FOUND);

    let disabled = snapshot_frame(None, Uuid::nil(), unique_id().as_str()).await;
    assert_eq!(status(disabled.unwrap_err()), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn unavailable_storage_is_a_503() {
    let storage_down = CommandStreamError::from(RedisError::RawError(redis::RedisError::from((
        redis::ErrorKind::InvalidClientConfig,
        "connection refused",
    ))));
    assert_eq!(problem_of(&storage_down).status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        problem_of(&CommandStreamError::NotReady).status,
        StatusCode::SERVICE_UNAVAILABLE
    );
}
