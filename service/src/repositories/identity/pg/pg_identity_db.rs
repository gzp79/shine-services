use std::future::Future;

use crate::repositories::{
    identity::{IdentityBuildError, IdentityDb, IdentityDbContext, IdentityError},
    DBError,
};
use shine_service::service::{PGConnectionPool, PGPooledConnection, PGTransaction};

use super::{
    PgExternalLinksStatements, PgIdSequencesStatements, PgIdentitiesStatements, PgRolesStatements, PgTokensStatements,
    PgVersionedUpdateStatements,
};

pub struct PgIdentityTransaction<'a> {
    pub transaction: PGTransaction<'a>,
    pub stmts_identities: PgIdentitiesStatements,
    pub stmts_external_links: PgExternalLinksStatements,
    pub stmts_tokens: PgTokensStatements,
    pub stmts_version: PgVersionedUpdateStatements,
    pub stmts_roles: PgRolesStatements,
    pub stmts_id_sequences: PgIdSequencesStatements,
}

pub struct PgIdentityDbContext<'c> {
    client: PGPooledConnection<'c>,
    stmts_identities: PgIdentitiesStatements,
    stmts_external_links: PgExternalLinksStatements,
    stmts_tokens: PgTokensStatements,
    stmts_version: PgVersionedUpdateStatements,
    stmts_roles: PgRolesStatements,
    stmts_id_sequences: PgIdSequencesStatements,
}

impl<'c> IdentityDbContext<'c> for PgIdentityDbContext<'c> {
    type Transaction<'a>
        = PgIdentityTransaction<'a>
    where
        Self: 'a;

    async fn begin_transaction(&mut self) -> Result<Self::Transaction<'_>, IdentityError> {
        let transaction = self.client.transaction().await.map_err(DBError::from)?;
        Ok(PgIdentityTransaction {
            transaction,
            stmts_identities: self.stmts_identities.clone(),
            stmts_external_links: self.stmts_external_links.clone(),
            stmts_tokens: self.stmts_tokens.clone(),
            stmts_version: self.stmts_version.clone(),
            stmts_roles: self.stmts_roles.clone(),
            stmts_id_sequences: self.stmts_id_sequences.clone(),
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
    stmts_id_sequences: PgIdSequencesStatements,
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
            stmts_id_sequences: PgIdSequencesStatements::new(&client).await?,
        })
    }
}

impl IdentityDb for PgIdentityDb {
    type Context<'a> = PgIdentityDbContext<'a>;

    async fn create_context(&self) -> Result<Self::Context<'_>, IdentityError> {
        let client = self.client.get().await.map_err(DBError::PGPoolError)?;
        Ok(PgIdentityDbContext {
            client,
            stmts_identities: self.stmts_identities.clone(),
            stmts_external_links: self.stmts_external_links.clone(),
            stmts_tokens: self.stmts_tokens.clone(),
            stmts_version: self.stmts_version.clone(),
            stmts_roles: self.stmts_roles.clone(),
            stmts_id_sequences: self.stmts_id_sequences.clone(),
        })
    }
}
