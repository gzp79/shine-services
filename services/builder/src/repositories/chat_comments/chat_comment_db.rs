use crate::{models::ChatError, repositories::chat_comments::ChatCommentStore};
use std::future::Future;

pub trait ChatCommentDbContext<'c>: ChatCommentStore + Send {}

pub trait ChatCommentDb: Send + Sync {
    fn create_context(&self) -> impl Future<Output = Result<impl ChatCommentDbContext<'_>, ChatError>> + Send;
}
