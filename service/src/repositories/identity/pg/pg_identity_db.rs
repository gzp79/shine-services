use crate::repositories::{
    identity::identity_db::{IdentityDbContext, IdentityDb},
    DBError, IdentityBuildError, IdentityError,
};
use shine_service::service::{PGConnectionPool, PGPooledConnection, PGTransaction};

use super::{
    PgExternalLinksStatements, PgIdentitiesStatements, PgRolesStatements, PgTokensStatements,
    PgVersionedUpdateStatements,
};

pub struct PgIdentityTransaction<'a> {
    pub transaction: PGTransaction<'a>,
    pub stmts_identities: &'a PgIdentitiesStatements,
    pub stmts_external_links: &'a PgExternalLinksStatements,
    pub stmts_tokens: &'a PgTokensStatements,
    pub stmts_version: &'a PgVersionedUpdateStatements,
    pub stmts_roles: &'a PgRolesStatements,
}

pub struct PgIdentityDbContext<'c> {
    client: PGPooledConnection<'c>,
    stmts_identities: &'c PgIdentitiesStatements,
    stmts_external_links: &'c PgExternalLinksStatements,
    stmts_tokens: &'c PgTokensStatements,
    stmts_version: &'c PgVersionedUpdateStatements,
    stmts_roles: &'c PgRolesStatements,
}

impl<'c> IdentityDbContext<'c> for PgIdentityDbContext<'c> {
    type Transaction<'a> = PgIdentityTransaction<'a> where 'c: 'a;

    async fn begin_transaction<'a>(&'a mut self) -> Result<Self::Transaction<'a>, IdentityError> {
        let transaction = self.client.transaction().await.map_err(DBError::from)?;
        Ok(PgIdentityTransaction {
            transaction,
            stmts_identities: self.stmts_identities,
            stmts_external_links: self.stmts_external_links,
            stmts_tokens: self.stmts_tokens,
            stmts_version: &self.stmts_version,
            stmts_roles: &self.stmts_roles,
        })
    }
}

pub struct PgIdentityDb {
    client: PGConnectionPool,
    stmts_identities: PgIdentitiesStatements,
    stmts_external_links: PgExternalLinksStatements,
    stmts_tokens: PgTokensStatements,
    stmts_version: PgVersionedUpdateStatements,
    stmts_roles: PgRolesStatements,
}

impl PgIdentityDb {
    pub async fn new(postgres: &PGConnectionPool) -> Result<Self, IdentityBuildError> {
        let client = postgres.get().await.map_err(DBError::PGPoolError)?;

        Ok(Self {
            client: postgres.clone(),
            stmts_identities: PgIdentitiesStatements::new(&client).await?,
            stmts_external_links: PgExternalLinksStatements::new(&client).await?,
            stmts_tokens: PgTokensStatements::new(&client).await?,
            stmts_version: PgVersionedUpdateStatements::new(&client).await?,
            stmts_roles: PgRolesStatements::new(&client).await?,
        })
    }
}

impl IdentityDb for PgIdentityDb {
    type Context<'c> = PgIdentityDbContext<'c>;

    async fn create_context(&self) -> Result<Self::Context<'_>, IdentityError> {
        let client = self.client.get().await.map_err(DBError::PGPoolError)?;
        Ok(PgIdentityDbContext {
            client,
            stmts_identities: &self.stmts_identities,
            stmts_external_links: &self.stmts_external_links,
            stmts_tokens: &self.stmts_tokens,
            stmts_version: &self.stmts_version,
            stmts_roles: &self.stmts_roles,
        })
    }
}
