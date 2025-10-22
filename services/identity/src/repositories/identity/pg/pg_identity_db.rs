use crate::repositories::{identity::{IdentityBuildError, IdentityDb, IdentityDbContext, IdentityError}, DBConfig};
use shine_infra::{crypto_utils::CryptoUtils, db::{DBError, PGConnectionPool, PGPooledConnection}};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

use super::{
    PgExternalLinksStatements, PgIdSequencesStatements, PgIdentitiesStatements, PgRolesStatements, PgTokensStatements,
};

pub struct PgIdentityDbContext<'c> {
    pub(in crate::repositories::identity::pg) client: PGPooledConnection<'c>,
    pub(in crate::repositories::identity::pg) crypto: &'c CryptoUtils,
    pub(in crate::repositories::identity::pg) stmts_identities: PgIdentitiesStatements,
    pub(in crate::repositories::identity::pg) stmts_external_links: PgExternalLinksStatements,
    pub(in crate::repositories::identity::pg) stmts_tokens: PgTokensStatements,
    pub(in crate::repositories::identity::pg) stmts_roles: PgRolesStatements,
    pub(in crate::repositories::identity::pg) stmts_id_sequences: PgIdSequencesStatements,
}

impl<'c> IdentityDbContext<'c> for PgIdentityDbContext<'c> {}

pub struct PgIdentityDb {
    client: PGConnectionPool,
    crypto: CryptoUtils,
    stmts_identities: PgIdentitiesStatements,
    stmts_external_links: PgExternalLinksStatements,
    stmts_tokens: PgTokensStatements,
    stmts_roles: PgRolesStatements,
    stmts_id_sequences: PgIdSequencesStatements,
}

impl PgIdentityDb {
    pub async fn new(postgres: &PGConnectionPool, config: &DBConfig) -> Result<Self, IdentityBuildError> {
        let client = postgres.get().await.map_err(DBError::PGPoolError)?;

        let email_config = &config.email_protection;
        let encryption_key = URL_SAFE_NO_PAD.decode(email_config.encryption_key.as_bytes())?;
        let hash_key = URL_SAFE_NO_PAD.decode(email_config.hash_key.as_bytes())?;
        let crypto = CryptoUtils::new(&encryption_key, &hash_key)?;


        Ok(Self {
            client: postgres.clone(),
            crypto,
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
            crypto: &self.crypto,
            stmts_identities: self.stmts_identities.clone(),
            stmts_external_links: self.stmts_external_links.clone(),
            stmts_tokens: self.stmts_tokens.clone(),
            stmts_roles: self.stmts_roles.clone(),
            stmts_id_sequences: self.stmts_id_sequences.clone(),
        })
    }
}
