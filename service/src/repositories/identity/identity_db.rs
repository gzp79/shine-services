use super::{ExternalLinks, IdSequences, Identities, IdentityError, IdentitySearch, Roles, Tokens};

pub trait IdentityDbContext<'c> {
    type Transaction<'a>: Identities + ExternalLinks + Roles + IdentitySearch + Tokens + IdSequences
    where
        Self: 'a;

    async fn begin_transaction(&mut self) -> Result<Self::Transaction<'_>, IdentityError>;
}

pub trait IdentityDb {
    type Context<'c>: IdentityDbContext<'c>
    where
        Self: 'c;

    async fn create_context(&self) -> Result<Self::Context<'_>, IdentityError>;
}
