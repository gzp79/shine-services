use crate::{
    models::ChatError,
    repositories::chat_comments::{ChatCommentStore, StoredChatComment},
};
use redis::streams::{StreamId, StreamRangeReply};
use shine_infra::db::redis::RedisError;
use uuid::Uuid;

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
    async fn append_comment(&mut self, room_id: &str, user_id: Uuid, text: &str) -> Result<String, ChatError> {
        let stream_key = self.to_stream_key(room_id);
        let stream_id: String = redis::cmd("XADD")
            .arg(&stream_key)
            .arg("*")
            .arg("user")
            .arg(user_id.to_string())
            .arg("text")
            .arg(text)
            .query_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)
            .map_err(ChatError::internal)?;

        let _: () = redis::pipe()
            .cmd("XTRIM")
            .arg(&stream_key)
            .arg("MAXLEN")
            .arg("~")
            .arg(self.max_messages)
            .ignore()
            .cmd("EXPIRE")
            .arg(&stream_key)
            .arg(self.ttl_seconds)
            .ignore()
            .cmd("PUBLISH")
            .arg(&stream_key)
            .arg(&stream_id)
            .ignore()
            .query_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)
            .map_err(ChatError::internal)?;

        Ok(stream_id)
    }

    async fn list_recent(&mut self, room_id: &str, limit: usize) -> Result<Vec<StoredChatComment>, ChatError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let stream_key = self.to_stream_key(room_id);
        let mut reply: StreamRangeReply = redis::cmd("XREVRANGE")
            .arg(&stream_key)
            .arg("+")
            .arg("-")
            .arg("COUNT")
            .arg(limit)
            .query_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)
            .map_err(ChatError::internal)?;

        let mut comments: Vec<StoredChatComment> = reply
            .ids
            .drain(..)
            .filter_map(|entry| self.parse_entry(entry))
            .collect();
        comments.reverse();
        Ok(comments)
    }

    async fn list_after(
        &mut self,
        room_id: &str,
        last_stream_id: &str,
        limit: usize,
    ) -> Result<Vec<StoredChatComment>, ChatError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let stream_key = self.to_stream_key(room_id);
        let start = format!("({last_stream_id}");
        let mut reply: StreamRangeReply = redis::cmd("XRANGE")
            .arg(&stream_key)
            .arg(start)
            .arg("+")
            .arg("COUNT")
            .arg(limit)
            .query_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)
            .map_err(ChatError::internal)?;

        Ok(reply
            .ids
            .drain(..)
            .filter_map(|entry| self.parse_entry(entry))
            .collect())
    }

    async fn find_by_stream_id(
        &mut self,
        room_id: &str,
        stream_id: &str,
    ) -> Result<Option<StoredChatComment>, ChatError> {
        let stream_key = self.to_stream_key(room_id);
        let mut reply: StreamRangeReply = redis::cmd("XRANGE")
            .arg(&stream_key)
            .arg(stream_id)
            .arg(stream_id)
            .arg("COUNT")
            .arg(1)
            .query_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)
            .map_err(ChatError::internal)?;

        let entry = reply.ids.drain(..).next();
        Ok(entry.and_then(|entry| self.parse_entry(entry)))
    }
}
