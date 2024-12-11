use std::future::Future;

use super::{SessionError, Sessions};

pub trait SessionDbContext<'c>: Send {
    type Transaction<'a>: Sessions + Send
    where
        Self: 'a;

    fn begin_transaction(&mut self) -> impl Future<Output = Result<Self::Transaction<'_>, SessionError>> + Send;
}

pub trait SessionDb: Send + Sync {
    type Context<'c>: SessionDbContext<'c>
    where
        Self: 'c;

    fn create_context(&self) -> impl Future<Output = Result<Self::Context<'_>, SessionError>> + Send;
}
