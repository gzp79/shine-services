use crate::{models::SessionError, repositories::session::Sessions};
use std::future::Future;

pub trait SessionDbContext<'c>: Sessions + Send {}

/// Access to the session data store.
pub trait SessionDb: Clone + Send + Sync {
    fn create_context(&self) -> impl Future<Output = Result<impl SessionDbContext<'_>, SessionError>> + Send;
}
