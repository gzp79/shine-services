use crate::{
    models::ChatError,
    repositories::chat_comments::{ChatCommentStore, ChatPage, StoredChatComment},
};
use redis::{
    streams::{StreamId, StreamRangeReply},
    Script,
};
use shine_infra::db::redis::RedisError;
use std::sync::LazyLock;
use tracing::instrument;
use uuid::Uuid;

/// Appends a comment, trims the stream, refreshes its TTL and announces the append in one atomic
/// step: a cancelled or failed caller can neither leave a stream without a TTL nor a stored comment
/// nobody was told about. `KEYS[1]` is the stream, `ARGV` the user, the text, the max length and the TTL.
const APPEND_LUA: &str = r#"
local id = redis.call('XADD', KEYS[1], '*', 'user', ARGV[1], 'text', ARGV[2])
redis.call('XTRIM', KEYS[1], 'MAXLEN', '~', ARGV[3])
redis.call('EXPIRE', KEYS[1], ARGV[4])
redis.call('PUBLISH', KEYS[1], id)
return id
"#;

static APPEND_SCRIPT: LazyLock<Script> = LazyLock::new(|| Script::new(APPEND_LUA));

use super::RedisChatCommentDbContext;

impl RedisChatCommentDbContext<'_> {
    fn to_stream_key(&self, room_id: &str) -> String {
        format!("chat:{room_id}")
    }

    fn parse_entry(&self, entry: StreamId) -> Option<StoredChatComment> {
        let user_id = entry.get::<String>("user").and_then(|raw| Uuid::parse_str(&raw).ok())?;
        let text = entry.get::<String>("text")?;

        Some(StoredChatComment {
            stream_id: entry.id,
            user_id,
            text,
        })
    }
}

impl ChatCommentStore for RedisChatCommentDbContext<'_> {
    #[instrument(level = "debug", skip_all, fields(room_id = %room_id, user_id = %user_id))]
    async fn append_comment(&mut self, room_id: &str, user_id: Uuid, text: &str) -> Result<String, ChatError> {
        APPEND_SCRIPT
            .key(self.to_stream_key(room_id))
            .arg(user_id.to_string())
            .arg(text)
            .arg(self.max_messages)
            .arg(self.ttl_seconds)
            .invoke_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)
            .map_err(ChatError::internal)
    }

    #[instrument(level = "debug", skip_all, fields(room_id = %room_id, limit))]
    async fn list_after(
        &mut self,
        room_id: &str,
        last_stream_id: &str,
        limit: usize,
    ) -> Result<ChatPage, ChatError> {
        if limit == 0 {
            return Ok(ChatPage::default());
        }

        let stream_key = self.to_stream_key(room_id);
        let start = format!("({last_stream_id}");
        let reply: StreamRangeReply = redis::cmd("XRANGE")
            .arg(&stream_key)
            .arg(start)
            .arg("+")
            .arg("COUNT")
            .arg(limit)
            .query_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)
            .map_err(ChatError::internal)?;

        let more = reply.ids.len() >= limit;
        let last_id = reply.ids.last().map(|entry| entry.id.clone());
        let comments = reply
            .ids
            .into_iter()
            .filter_map(|entry| self.parse_entry(entry))
            .collect();
        Ok(ChatPage { comments, last_id, more })
    }

    #[instrument(level = "debug", skip_all, fields(room_id = %room_id))]
    async fn last_id(&mut self, room_id: &str) -> Result<Option<String>, ChatError> {
        let reply: StreamRangeReply = redis::cmd("XREVRANGE")
            .arg(self.to_stream_key(room_id))
            .arg("+")
            .arg("-")
            .arg("COUNT")
            .arg(1)
            .query_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)
            .map_err(ChatError::internal)?;

        Ok(reply.ids.into_iter().next().map(|entry| entry.id))
    }
}
