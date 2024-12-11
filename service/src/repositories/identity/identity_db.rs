use std::future::Future;

use super::{ExternalLinks, IdSequences, Identities, IdentityError, IdentitySearch, Roles, Tokens};

pub trait IdentityDbContext<'c>: Send {
    type Transaction<'a>: Identities + ExternalLinks + Roles + IdentitySearch + Tokens + IdSequences + Send
    where
        Self: 'a;

    fn begin_transaction(&mut self)
        -> impl Future<Output = Result<Self::Transaction<'_>, IdentityError>> + Send;
}

pub trait IdentityDb: Send + Sync {
    type Context<'a>: IdentityDbContext<'a>
    where
        Self: 'a;

    fn create_context(&self) -> impl Future<Output = Result<Self::Context<'_>, IdentityError>> + Send;
}
