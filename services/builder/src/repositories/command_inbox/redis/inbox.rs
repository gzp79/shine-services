use crate::repositories::command_inbox::{
    redis::{wake::WAKE_CHANNEL, RedisCommandInboxDb, RedisCommandInboxError},
    InboxBatch, InboxEntry, InboxId,
};
use redis::streams::{StreamId as StreamEntry, StreamRangeReply};
use shine_game::replica::StreamId;
use shine_infra::db::redis::RedisError;
use tracing::instrument;
use uuid::Uuid;

/// One transaction, so a command is never in the inbox without an expiry, nor announced without
/// being there.
#[instrument(level = "debug", skip_all, fields(stream_id = %stream_id, command_id = %command_id))]
pub async fn push(
    db: &RedisCommandInboxDb,
    stream_id: &StreamId,
    command_id: Uuid,
    user_id: Uuid,
    data: &str,
) -> Result<InboxId, RedisCommandInboxError> {
    let key = RedisCommandInboxDb::stream_key(stream_id);
    let mut client = db.client().await?;

    let (id,): (String,) = redis::pipe()
        .atomic()
        .cmd("XADD")
        .arg(&key)
        .arg("*")
        .arg("command_id")
        .arg(command_id.to_string())
        .arg("user_id")
        .arg(user_id.to_string())
        .arg("data")
        .arg(data)
        .cmd("EXPIRE")
        .arg(&key)
        .arg(db.ttl_seconds())
        .ignore()
        .cmd("PUBLISH")
        .arg(WAKE_CHANNEL)
        .arg(stream_id.to_string())
        .ignore()
        .query_async(&mut *client)
        .await
        .map_err(RedisError::RawError)?;

    InboxId::parse(&id).ok_or_else(|| RedisCommandInboxError::Malformed(format!("stream id {id:?}")))
}

#[instrument(level = "debug", skip_all, fields(stream_id = %stream_id, limit))]
pub async fn poll(
    db: &RedisCommandInboxDb,
    stream_id: &StreamId,
    after: &InboxId,
    limit: usize,
) -> Result<InboxBatch, RedisCommandInboxError> {
    let mut batch = InboxBatch {
        entries: Vec::new(),
        cursor: after.clone(),
        more: false,
    };
    if limit == 0 {
        return Ok(batch);
    }

    let mut client = db.client().await?;
    // The exclusive range needs Redis 6.2.
    let reply: StreamRangeReply = redis::cmd("XRANGE")
        .arg(RedisCommandInboxDb::stream_key(stream_id))
        .arg(format!("({after}"))
        .arg("+")
        .arg("COUNT")
        .arg(limit)
        .query_async(&mut *client)
        .await
        .map_err(RedisError::RawError)?;

    batch.more = reply.ids.len() >= limit;
    for raw in reply.ids {
        let Some(inbox_id) = InboxId::parse(&raw.id) else {
            continue;
        };
        match parse_entry(&raw, inbox_id.clone()) {
            Some(entry) => batch.entries.push(entry),
            None => tracing::warn!("[{stream_id}] Skipping unreadable inbox entry {inbox_id}"),
        }
        batch.cursor = inbox_id;
    }
    Ok(batch)
}

#[instrument(level = "debug", skip_all, fields(stream_id = %stream_id, up_to = %up_to))]
pub async fn trim(
    db: &RedisCommandInboxDb,
    stream_id: &StreamId,
    up_to: &InboxId,
) -> Result<usize, RedisCommandInboxError> {
    let mut client = db.client().await?;
    // MINID evicts the entries below the given id: one above `up_to` keeps `up_to` out.
    let removed: usize = redis::cmd("XTRIM")
        .arg(RedisCommandInboxDb::stream_key(stream_id))
        .arg("MINID")
        .arg(up_to.next().to_string())
        .query_async(&mut *client)
        .await
        .map_err(RedisError::RawError)?;
    Ok(removed)
}

fn parse_entry(raw: &StreamEntry, inbox_id: InboxId) -> Option<InboxEntry> {
    let command_id = raw
        .get::<String>("command_id")
        .and_then(|id| Uuid::parse_str(&id).ok())?;
    let user_id = raw.get::<String>("user_id").and_then(|id| Uuid::parse_str(&id).ok())?;
    let data = raw.get::<String>("data")?;
    Some(InboxEntry {
        inbox_id,
        command_id,
        user_id,
        data,
    })
}
