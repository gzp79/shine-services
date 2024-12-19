use std::future::Future;

use super::{ExternalLinks, IdSequences, Identities, IdentityError, IdentitySearch, Roles, Tokens};

pub trait IdentityDbContext<'c>:
    Identities + ExternalLinks + Roles + IdentitySearch + Tokens + IdSequences + Send
{
}

pub trait IdentityDb: Send + Sync {
    fn create_context(&self) -> impl Future<Output = Result<impl IdentityDbContext<'_>, IdentityError>> + Send;
}
