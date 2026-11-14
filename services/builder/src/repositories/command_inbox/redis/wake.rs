use crate::repositories::command_inbox::{
    redis::{RedisCommandInboxDb, RedisCommandInboxError},
    InboxWake,
};
use shine_game::replica::StreamId;
use shine_infra::db::redis::RedisError;
use tracing::instrument;

/// One channel for every stream: a subscription is a network round trip, and a listener has to
/// reconnect them all, so a channel per stream would make a connection loss slow in proportion to
/// the streams. The payload is `<stream id>` for a command, `<stream id>@<version>` for a persisted
/// version (`@` is not a character of a stream id).
pub const WAKE_CHANNEL: &str = "cmd-inbox-wake";

#[instrument(level = "debug", skip_all, fields(stream_id = %stream_id, version))]
pub async fn notify_persisted(
    db: &RedisCommandInboxDb,
    stream_id: &StreamId,
    version: i64,
) -> Result<(), RedisCommandInboxError> {
    let mut client = db.client().await?;
    let _: i64 = redis::cmd("PUBLISH")
        .arg(WAKE_CHANNEL)
        .arg(format!("{stream_id}@{version}"))
        .query_async(&mut *client)
        .await
        .map_err(RedisError::RawError)?;
    Ok(())
}

pub async fn listen<F>(db: &RedisCommandInboxDb, handler: F) -> Result<(), RedisCommandInboxError>
where
    F: Fn(InboxWake) + Send + Sync + 'static,
{
    let client = db.client().await?;
    client
        .listen(WAKE_CHANNEL, move |payload| match payload {
            // A reconnect: the wakes published meanwhile are lost.
            None => handler(InboxWake::All),
            Some(payload) => match parse_payload(payload) {
                Some(wake) => handler(wake),
                None => tracing::error!("Malformed command inbox wake {payload:?}"),
            },
        })
        .await?;
    Ok(())
}

pub async fn unlisten(db: &RedisCommandInboxDb) -> Result<(), RedisCommandInboxError> {
    let client = db.client().await?;
    client.unlisten(WAKE_CHANNEL).await?;
    Ok(())
}

fn parse_payload(payload: &str) -> Option<InboxWake> {
    let (stream_id, persisted) = match payload.split_once('@') {
        Some((stream_id, version)) => (stream_id, Some(version.parse().ok()?)),
        None => (payload, None),
    };
    Some(InboxWake::Stream {
        stream_id: StreamId::parse(stream_id).ok()?,
        persisted,
    })
}
