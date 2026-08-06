use crate::{
    models::IdentityError,
    repositories::{
        identity::{pg::PgIdentityBuildError, IdentityDb, IdentityDbContext},
        EmailProtectionConfig,
    },
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use shine_infra::{
    crypto::DataProtectionUtils,
    db::postgres::{PgConnectionPool, PgError, PgPooledConnection},
};
use std::sync::Arc;

use super::{
    PgExternalLinksStatements, PgIdSequencesStatements, PgIdentitiesStatements, PgRolesStatements, PgTokensStatements,
};

pub struct PgIdentityDbContext<'c> {
    pub(in crate::repositories::identity::pg) client: PgPooledConnection<'c>,
    pub(in crate::repositories::identity::pg) email_protection: &'c DataProtectionUtils,
    pub(in crate::repositories::identity::pg) stmts_identities: PgIdentitiesStatements,
    pub(in crate::repositories::identity::pg) stmts_external_links: PgExternalLinksStatements,
    pub(in crate::repositories::identity::pg) stmts_tokens: PgTokensStatements,
    pub(in crate::repositories::identity::pg) stmts_roles: PgRolesStatements,
    pub(in crate::repositories::identity::pg) stmts_id_sequences: PgIdSequencesStatements,
}

impl<'c> IdentityDbContext<'c> for PgIdentityDbContext<'c> {}

struct Inner {
    client: PgConnectionPool,
    email_protection: DataProtectionUtils,
    stmts_identities: PgIdentitiesStatements,
    stmts_external_links: PgExternalLinksStatements,
    stmts_tokens: PgTokensStatements,
    stmts_roles: PgRolesStatements,
    stmts_id_sequences: PgIdSequencesStatements,
}

/// Cheaply cloneable handle to the prepared statement set and connection pool.
#[derive(Clone)]
pub struct PgIdentityDb(Arc<Inner>);

impl PgIdentityDb {
    pub async fn new(
        postgres: &PgConnectionPool,
        config: &EmailProtectionConfig,
    ) -> Result<Self, PgIdentityBuildError> {
        let client = postgres.get().await.map_err(PgError::PoolError)?;

        let encryption_key = B64.decode(config.encryption_key.as_bytes())?;
        let hash_key = B64.decode(config.hash_key.as_bytes())?;
        let email_protection = DataProtectionUtils::new(&encryption_key, &hash_key)?;

        Ok(Self(Arc::new(Inner {
            client: postgres.clone(),
            email_protection,
            stmts_identities: PgIdentitiesStatements::new(&client).await?,
            stmts_external_links: PgExternalLinksStatements::new(&client).await?,
            stmts_tokens: PgTokensStatements::new(&client).await?,
            stmts_roles: PgRolesStatements::new(&client).await?,
            stmts_id_sequences: PgIdSequencesStatements::new(&client).await?,
        })))
    }
}

impl IdentityDb for PgIdentityDb {
    async fn create_context(&self) -> Result<impl IdentityDbContext<'_>, IdentityError> {
        let client = self.0.client.get().await.map_err(PgError::PoolError)?;
        Ok(PgIdentityDbContext {
            client,
            email_protection: &self.0.email_protection,
            stmts_identities: self.0.stmts_identities.clone(),
            stmts_external_links: self.0.stmts_external_links.clone(),
            stmts_tokens: self.0.stmts_tokens.clone(),
            stmts_roles: self.0.stmts_roles.clone(),
            stmts_id_sequences: self.0.stmts_id_sequences.clone(),
        })
    }
}
