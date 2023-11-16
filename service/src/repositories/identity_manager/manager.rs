use crate::repositories::{
    DBError, ExternalLink, ExternalUserInfo, Identity, IdentityBuildError, IdentityError, TokenInfo, TokenKind,
};
use chrono::Duration;
use ring::digest;
use shine_service::{
    axum::SiteInfo,
    service::{ClientFingerprint, PGConnectionPool},
};
use std::sync::Arc;
use uuid::Uuid;

use super::{
    external_links::{ExternalLinks, ExternalLinksStatements},
    identities::{Identities, IdentitiesStatements},
    roles::{Roles, RolesStatements},
    search_identities::{IdentitySearch, SearchIdentity},
    tokens::{Tokens, TokensStatements},
    versioned_update::VersionedUpdateStatements,
};

struct Inner {
    postgres: PGConnectionPool,
    stmts_identities: IdentitiesStatements,
    stmts_external_links: ExternalLinksStatements,
    stmts_tokens: TokensStatements,
    stmts_version: VersionedUpdateStatements,
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
            stmts_version: VersionedUpdateStatements::new(&client).await?,
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

        let mut identities_dao = Identities::new(&client, &inner.stmts_identities);
        let mut external_links_dao = ExternalLinks::new(&client, &inner.stmts_external_links);

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

        Identities::new(&client, &inner.stmts_identities)
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

        ExternalLinks::new(&client, &inner.stmts_external_links)
            .find_by_external_link(provider, provider_id)
            .await
    }

    pub async fn search(&self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        IdentitySearch::new(&client).search(search).await
    }

    pub async fn cascaded_delete(&self, user_id: Uuid) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        Identities::new(&client, &inner.stmts_identities)
            .cascaded_delete(user_id)
            .await
    }

    pub async fn link_user(&self, user_id: Uuid, external_user: &ExternalUserInfo) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        ExternalLinks::new(&client, &inner.stmts_external_links)
            .link_user(user_id, external_user)
            .await
    }

    pub async fn unlink_user(
        &self,
        user_id: Uuid,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<()>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        ExternalLinks::new(&client, &inner.stmts_external_links)
            .delete_link(user_id, provider, provider_id)
            .await
    }

    pub async fn list_find_links(&self, user_id: Uuid) -> Result<Vec<ExternalLink>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        ExternalLinks::new(&client, &inner.stmts_external_links)
            .find_all(user_id)
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
    ) -> Result<TokenInfo, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        let token_hash = hash_token(token);
        Tokens::new(&client, &inner.stmts_tokens)
            .store_token(user_id, &token_hash, duration, fingerprint, site_info, kind)
            .await
    }

    pub async fn find_token(&self, token: &str) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let token_hash = hash_token(token);
        self.find_token_hash(&token_hash).await
    }

    pub async fn find_token_hash(&self, token_hash: &str) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        Tokens::new(&client, &inner.stmts_tokens).find_token(token_hash).await
    }

    pub async fn list_all_tokens(&self, user_id: &Uuid) -> Result<Vec<TokenInfo>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        Tokens::new(&client, &inner.stmts_tokens).find_by_user(user_id).await
    }

    pub async fn update_token(&self, token: &str, duration: &Duration) -> Result<TokenInfo, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        let token_hash = hash_token(token);
        Tokens::new(&client, &inner.stmts_tokens)
            .update_token(&token_hash, duration)
            .await
    }

    pub async fn delete_token(&self, user_id: Uuid, token: &str) -> Result<Option<()>, IdentityError> {
        let token_hash = hash_token(token);
        self.delete_token_hash(user_id, &token_hash).await
    }

    pub async fn delete_token_hash(&self, user_id: Uuid, token_hash: &str) -> Result<Option<()>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        Tokens::new(&client, &inner.stmts_tokens)
            .delete_token(user_id, token_hash)
            .await
    }

    pub async fn delete_all_tokens(&self, user_id: Uuid, kinds: &[TokenKind]) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        Tokens::new(&client, &inner.stmts_tokens)
            .delete_all_tokens(user_id, kinds)
            .await
    }

    pub async fn add_role(&self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError> {
        let inner = &*self.0;
        let mut client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        Roles::new(&mut client, &inner.stmts_version, &inner.stmts_roles)
            .add_role(user_id, role)
            .await
    }

    pub async fn get_roles(&self, user_id: Uuid) -> Result<Option<Vec<String>>, IdentityError> {
        let inner = &*self.0;
        let mut client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        Roles::new(&mut client, &inner.stmts_version, &inner.stmts_roles)
            .get_roles(user_id)
            .await
    }

    pub async fn delete_role(&self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError> {
        let inner = &*self.0;
        let mut client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        Roles::new(&mut client, &inner.stmts_version, &inner.stmts_roles)
            .delete_role(user_id, role)
            .await
    }
}

/// Generate a (crypto) hashed version of a token to protect data in rest.
fn hash_token(token: &str) -> String {
    // there is no need for a complex hash as key has a big entropy already
    // and it'd be too expensive to invert the hashing.
    let hash = digest::digest(&digest::SHA256, token.as_bytes());
    let hash = hex::encode(hash);
    log::debug!("Hashing token: {token:?} -> [{hash}]");
    hash
}
