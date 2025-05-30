use crate::repositories::identity::{
    IdentityBuildError, IdentityDb, IdentityDbContext, IdentityError,
};
use shine_infra::db::{DBError, PGConnectionPool, PGPooledConnection};

use super::{
    PgExternalLinksStatements, PgIdSequencesStatements, PgIdentitiesStatements, PgRolesStatements,
    PgTokensStatements,
};

pub struct PgIdentityDbContext<'c> {
    pub(in crate::repositories::identity::pg) client: PGPooledConnection<'c>,
    pub(in crate::repositories::identity::pg) stmts_identities: PgIdentitiesStatements,
    pub(in crate::repositories::identity::pg) stmts_external_links: PgExternalLinksStatements,
    pub(in crate::repositories::identity::pg) stmts_tokens: PgTokensStatements,
    pub(in crate::repositories::identity::pg) stmts_roles: PgRolesStatements,
    pub(in crate::repositories::identity::pg) stmts_id_sequences: PgIdSequencesStatements,
}

impl<'c> IdentityDbContext<'c> for PgIdentityDbContext<'c> {}

pub struct PgIdentityDb {
    client: PGConnectionPool,
    stmts_identities: PgIdentitiesStatements,
    stmts_external_links: PgExternalLinksStatements,
    stmts_tokens: PgTokensStatements,
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
            stmts_roles: PgRolesStatements::new(&client).await?,
            stmts_id_sequences: PgIdSequencesStatements::new(&client).await?,
        })
    }
}

impl IdentityDb for PgIdentityDb {
    async fn create_context(&self) -> Result<impl IdentityDbContext<'_>, IdentityError> {
        let client = self.client.get().await.map_err(DBError::PGPoolError)?;
        Ok(PgIdentityDbContext {
            client,
            stmts_identities: self.stmts_identities.clone(),
            stmts_external_links: self.stmts_external_links.clone(),
            stmts_tokens: self.stmts_tokens.clone(),
            stmts_roles: self.stmts_roles.clone(),
            stmts_id_sequences: self.stmts_id_sequences.clone(),
        })
    }
}
