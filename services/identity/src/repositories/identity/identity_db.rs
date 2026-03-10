use crate::{
    models::IdentityError,
    repositories::identity::{ExternalLinks, IdSequences, Identities, IdentitySearch, Roles, Tokens},
};
use std::future::Future;

pub trait IdentityDbContext<'c>:
    Identities + ExternalLinks + Roles + IdentitySearch + Tokens + IdSequences + Send
{
}

pub trait IdentityDb: Send + Sync {
    fn create_context(&self) -> impl Future<Output = Result<impl IdentityDbContext<'_>, IdentityError>> + Send;
}
