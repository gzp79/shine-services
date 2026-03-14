use crate::{
    models::{Identity, IdentityError, TokenInfo, TokenKind},
    repositories::identity::{IdentityDb, Tokens},
};
use chrono::Duration;
use ring::digest;
use shine_infra::web::extracts::{ClientFingerprint, SiteInfo};
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum TokenError {
    #[error("Retry limit reached for token creation")]
    RetryLimitReached,
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

pub struct TokenService<DB: IdentityDb> {
    db: DB,
}

impl<DB: IdentityDb> TokenService<DB> {
    pub fn new(db: DB) -> Self {
        Self { db }
    }

    pub async fn find_by_hash(&self, token_hash: &str) -> Result<Option<TokenInfo>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.find_by_hash(token_hash).await
    }

    pub async fn list_by_user(&self, user_id: &Uuid) -> Result<Vec<TokenInfo>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.find_by_user(user_id).await
    }

    pub async fn test(
        &self,
        allowed_kinds: &[TokenKind],
        token: &str,
    ) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        let token_hash = hash_token(token);
        ctx.test_token(allowed_kinds, &token_hash).await
    }

    pub async fn take(
        &self,
        allowed_kinds: &[TokenKind],
        token: &str,
    ) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        let token_hash = hash_token(token);
        ctx.take_token(allowed_kinds, &token_hash).await
    }

    pub async fn delete(&self, kind: TokenKind, token: &str) -> Result<Option<()>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        let token_hash = hash_token(token);
        ctx.delete_token_by_hash(kind, &token_hash).await
    }

    pub async fn delete_by_user(&self, user_id: Uuid, token_hash: &str) -> Result<Option<()>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.delete_token_by_user(user_id, token_hash).await
    }

    pub async fn delete_all_by_user(&self, user_id: Uuid, kinds: &[TokenKind]) -> Result<(), IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.delete_all_token_by_user(user_id, kinds).await
    }

    pub async fn create_with_retry(
        &self,
        user_id: Uuid,
        kind: TokenKind,
        ttl: &Duration,
        fingerprint: Option<&ClientFingerprint>,
        email: Option<&str>,
        site_info: &SiteInfo,
    ) -> Result<(String, TokenInfo), TokenError> {
        const MAX_RETRY_COUNT: usize = 10;

        let mut retry_count = 0;
        loop {
            log::debug!("Creating new token; retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(TokenError::RetryLimitReached);
            }
            retry_count += 1;

            let token = Uuid::new_v4().to_string();
            let token_hash = hash_token(&token);

            // Business rule: If token kind must be unique per user, delete old tokens first
            if kind.is_unique() {
                self.delete_all_by_user(user_id, &[kind]).await?;
            }

            let mut ctx = self.db.create_context().await?;
            match ctx
                .store_token(user_id, kind, &token_hash, ttl, fingerprint, email, site_info)
                .await
            {
                Ok(token_info) => return Ok((token, token_info)),
                Err(IdentityError::TokenConflict) => continue,
                Err(err) => return Err(TokenError::IdentityError(err)),
            }
        }
    }
}

/// Generate a (crypto) hashed version of a token to protect data at rest.
fn hash_token(token: &str) -> String {
    let hash = digest::digest(&digest::SHA256, token.as_bytes());
    let hash = hex::encode(hash);
    log::debug!("Hashing token: {token:?} -> [{hash}]");
    hash
}
