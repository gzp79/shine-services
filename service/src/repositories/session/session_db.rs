use super::{session_error::SessionError, sessions::Sessions};

pub trait SessionDbContext<'c> {
    type Transaction<'a>: Sessions
    where
        Self: 'a;

    async fn begin_transaction(&mut self) -> Result<Self::Transaction<'_>, SessionError>;
}

pub trait SessionDb {
    type Context<'c>: SessionDbContext<'c>
    where
        Self: 'c;

    async fn create_context(&self) -> Result<Self::Context<'_>, SessionError>;
}
