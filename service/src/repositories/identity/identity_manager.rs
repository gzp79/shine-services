use chrono::Duration;
use shine_service::{axum::SiteInfo, service::ClientFingerprint};
use uuid::Uuid;

use super::{
    external_links::{ExternalLink, ExternalLinks, ExternalUserInfo},
    hash_token,
    identities::{Identities, Identity},
    identity_db::{IdentityDbContext, IdentityDb},
    identity_error::IdentityError,
    roles::Roles,
    search_identities::IdentitySearch,
    tokens::Tokens,
    SearchIdentity, TokenInfo, TokenKind,
};

#[derive(Clone)]
pub struct IdentityManager<DB: IdentityDb + Clone>(DB);

impl<DB> IdentityManager<DB>
where
    DB: IdentityDb + Clone,
{
    pub fn new(db: DB) -> Self {
        Self(db)
    }

    pub async fn create_user(
        &self,
        user_id: Uuid,
        user_name: &str,
        email: Option<&str>,
        external_user_info: Option<&ExternalUserInfo>,
    ) -> Result<Identity, IdentityError> {
        //let email = email.map(|e| e.normalize_email());
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let identity = transaction.create_user(user_id, user_name, email).await?;
        if let Some(external_user_info) = external_user_info {
            if let Err(err) = transaction.link_user(user_id, external_user_info).await {
                if let Err(err) = transaction.cascaded_delete(user_id).await {
                    log::error!("Failed to delete user ({}) after failed link: {}", user_id, err);
                }
                return Err(err);
            }
        }
        Ok(identity)
    }

    pub async fn find_by_id(&self, user_id: Uuid) -> Result<Option<Identity>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.find_by_id(user_id).await
    }

    pub async fn cascaded_delete(&self, user_id: Uuid) -> Result<(), IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.cascaded_delete(user_id).await
    }

    pub async fn find_by_external_link(
        &self,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<Identity>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.find_by_external_link(provider, provider_id).await
    }

    pub async fn link_user(&self, user_id: Uuid, external_user: &ExternalUserInfo) -> Result<(), IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.link_user(user_id, external_user).await
    }

    pub async fn delete_link(
        &self,
        user_id: Uuid,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<()>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.delete_link(user_id, provider, provider_id).await
    }

    pub async fn is_linked(&self, user_id: Uuid) -> Result<bool, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.is_linked(user_id).await
    }

    pub async fn find_all_links(&self, user_id: Uuid) -> Result<Vec<ExternalLink>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.find_all_links(user_id).await
    }

    pub async fn search(&self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.search_identity(search).await
    }

    pub async fn add_token(
        &self,
        user_id: Uuid,
        kind: TokenKind,
        token: &str,
        time_to_live: &Duration,
        fingerprint: Option<&ClientFingerprint>,
        site_info: &SiteInfo,
    ) -> Result<TokenInfo, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let token_hash = hash_token(token);
        transaction
            .store_token(user_id, kind, &token_hash, time_to_live, fingerprint, site_info)
            .await
    }

    pub async fn find_token_by_hash(&self, token_hash: &str) -> Result<Option<TokenInfo>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.find_by_hash(token_hash).await
    }

    pub async fn list_all_tokens_by_user(&self, user_id: &Uuid) -> Result<Vec<TokenInfo>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.find_by_user(user_id).await
    }

    /// Get the identity associated to an access token.
    /// The provided token is not removed from the DB.
    pub async fn test_access_token(&self, token: &str) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let token_hash = hash_token(token);
        transaction.test_token(TokenKind::Access, &token_hash).await
    }

    /// Get the identity associated to an api key.
    /// The provided token is not removed from the DB.
    pub async fn test_api_key(&self, token: &str) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let token_hash = hash_token(token);
        transaction.test_token(TokenKind::Persistent, &token_hash).await
    }

    /// Get the identity associated to a single access token.
    /// Independent of the result the provided toke is removed from the DB
    pub async fn take_single_access_token(&self, token: &str) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let token_hash = hash_token(token);
        transaction.take_token(TokenKind::SingleAccess, &token_hash).await
    }

    pub async fn delete_access_token(&self, token: &str) -> Result<Option<()>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let token_hash = hash_token(token);
        transaction.delete_token_by_hash(TokenKind::Access, &token_hash).await
    }

    pub async fn delete_persistent_token(&self, token: &str) -> Result<Option<()>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let token_hash = hash_token(token);
        transaction
            .delete_token_by_hash(TokenKind::Persistent, &token_hash)
            .await
    }

    pub async fn delete_token(&self, user_id: Uuid, token: &str) -> Result<Option<()>, IdentityError> {
        let token_hash = hash_token(token);
        self.delete_token_by_user(user_id, &token_hash).await
    }

    pub async fn delete_token_by_user(&self, user_id: Uuid, token_hash: &str) -> Result<Option<()>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.delete_token_by_user(user_id, token_hash).await
    }

    pub async fn delete_all_tokens_by_user(&self, user_id: Uuid, kinds: &[TokenKind]) -> Result<(), IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.delete_all_token_by_user(user_id, kinds).await
    }

    pub async fn add_role(&self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;
        transaction.add_role(user_id, role).await
    }

    async fn get_roles(&self, user_id: Uuid) -> Result<Option<Vec<String>>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;
        transaction.get_roles(user_id).await
    }

    async fn delete_role(&self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError> {
        let mut db = self.0.create_context().await?;
        let mut transaction = db.begin_transaction().await?;
        transaction.delete_role(user_id, role).await
    }
}
