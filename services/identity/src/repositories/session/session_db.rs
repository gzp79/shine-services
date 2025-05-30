use std::future::Future;

use super::{SessionError, Sessions};

pub trait SessionDbContext<'c>: Sessions + Send {}

pub trait SessionDb: Send + Sync {
    fn create_context(
        &self,
    ) -> impl Future<Output = Result<impl SessionDbContext<'_>, SessionError>> + Send;
}
