use crate::db::{
    CurrentToken, DBError, ExternalLinksDAO, ExternalLinksStatements, ExternalUserInfo, IdentitiesStatements, Identity,
    IdentityBuildError, IdentityError, IdentitySearchDAO, IdentityVersionStatements, RolesDAO, RolesStatements,
    SiteInfo, TokenKind, TokensDAO, TokensStatements,
};
use chrono::Duration;
use shine_service::service::{ClientFingerprint, PGConnectionPool};
use std::sync::Arc;
use uuid::Uuid;

use super::{IdentitiesDAO, SearchIdentity};

struct Inner {
    postgres: PGConnectionPool,
    stmts_identities: IdentitiesStatements,
    stmts_external_links: ExternalLinksStatements,
    stmts_tokens: TokensStatements,
    stmts_version: IdentityVersionStatements,
    stmts_roles: RolesStatements,
}

#[derive(Clone)]
pub struct IdentityManager(Arc<Inner>);

impl IdentityManager {
    pub async fn new(postgres: &PGConnectionPool) -> Result<Self, IdentityBuildError> {
        let client = postgres.get().await.map_err(DBError::PGPoolError)?;

        Ok(Self(Arc::new(Inner {
            postgres: postgres.clone(),
            stmts_identities: IdentitiesStatements::new(&client).await?,
            stmts_external_links: ExternalLinksStatements::new(&client).await?,
            stmts_tokens: TokensStatements::new(&client).await?,
            stmts_version: IdentityVersionStatements::new(&client).await?,
            stmts_roles: RolesStatements::new(&client).await?,
        })))
    }

    pub async fn create_user(
        &self,
        user_id: Uuid,
        user_name: &str,
        email: Option<&str>,
        external_login: Option<&ExternalUserInfo>,
    ) -> Result<Identity, IdentityError> {
        //let email = email.map(|e| e.normalize_email());
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        let mut identities_dao = IdentitiesDAO::new(&client, &inner.stmts_identities);
        let mut external_links_dao = ExternalLinksDAO::new(&client, &inner.stmts_external_links);

        let identity = identities_dao.create_user(user_id, user_name, email).await?;
        if let Some(external_login) = external_login {
            if let Err(err) = external_links_dao.link_user(user_id, external_login).await {
                if let Err(err) = identities_dao.cascaded_delete(user_id).await {
                    log::error!("Failed to delete user ({}) after failed link: {}", user_id, err);
                }
                return Err(err);
            }
        }
        Ok(identity)
    }

    pub async fn find_by_id(&self, user_id: Uuid) -> Result<Option<Identity>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        IdentitiesDAO::new(&client, &inner.stmts_identities)
            .find_by_id(user_id)
            .await
    }

    pub async fn find_by_external_link(
        &self,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<Identity>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        ExternalLinksDAO::new(&client, &inner.stmts_external_links)
            .find_by_external_link(provider, provider_id)
            .await
    }

    pub async fn search(&self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        IdentitySearchDAO::new(&client).search(search).await
    }

    pub async fn cascaded_delete(&self, user_id: Uuid) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        IdentitiesDAO::new(&client, &inner.stmts_identities)
            .cascaded_delete(user_id)
            .await
    }

    pub async fn link_user(&self, user_id: Uuid, external_user: &ExternalUserInfo) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        ExternalLinksDAO::new(&client, &inner.stmts_external_links)
            .link_user(user_id, external_user)
            .await
    }

    pub async fn create_token(
        &self,
        user_id: Uuid,
        token: &str,
        duration: &Duration,
        fingerprint: Option<&ClientFingerprint>,
        site_info: &SiteInfo,
        kind: TokenKind,
    ) -> Result<CurrentToken, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        TokensDAO::new(&client, &inner.stmts_tokens)
            .create_token(user_id, token, duration, fingerprint, site_info, kind)
            .await
    }

    pub async fn find_token(&self, token: &str) -> Result<Option<(Identity, CurrentToken)>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        TokensDAO::new(&client, &inner.stmts_tokens).find_token(token).await
    }

    pub async fn update_token(&self, token: &str, duration: &Duration) -> Result<CurrentToken, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        TokensDAO::new(&client, &inner.stmts_tokens)
            .update_token(token, duration)
            .await
    }

    pub async fn delete_token(&self, user_id: Uuid, token: &str) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        TokensDAO::new(&client, &inner.stmts_tokens)
            .delete_token(user_id, token)
            .await
    }

    pub async fn delete_all_tokens(&self, user_id: Uuid, kinds: &[TokenKind]) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        TokensDAO::new(&client, &inner.stmts_tokens)
            .delete_all_tokens(user_id, kinds)
            .await
    }

    pub async fn add_role(&self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError> {
        let inner = &*self.0;
        let mut client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        RolesDAO::new(&mut client, &inner.stmts_version, &inner.stmts_roles)
            .add_role(user_id, role)
            .await
    }

    pub async fn get_roles(&self, user_id: Uuid) -> Result<Option<Vec<String>>, IdentityError> {
        let inner = &*self.0;
        let mut client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        RolesDAO::new(&mut client, &inner.stmts_version, &inner.stmts_roles)
            .get_roles(user_id)
            .await
    }

    pub async fn delete_role(&self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError> {
        let inner = &*self.0;
        let mut client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        RolesDAO::new(&mut client, &inner.stmts_version, &inner.stmts_roles)
            .delete_role(user_id, role)
            .await
    }
}
