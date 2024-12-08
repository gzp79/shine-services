use chrono::{DateTime, Duration, Utc};
use ring::digest;
use serde::{Deserialize, Serialize};
use shine_service::{axum::SiteInfo, service::ClientFingerprint};
use uuid::Uuid;

use super::{Identity, IdentityError};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TokenKind {
    SingleAccess,
    Persistent,
    Access,
}

#[derive(Debug)]
pub struct TokenInfo {
    pub user_id: Uuid,
    pub kind: TokenKind,
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub expire_at: DateTime<Utc>,
    pub is_expired: bool,
    pub fingerprint: Option<String>,
    pub agent: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

/// Handle tokens
pub trait Tokens {
    async fn store_token(
        &mut self,
        user_id: Uuid,
        kind: TokenKind,
        token_hash: &str,
        time_to_live: &Duration,
        fingerprint: Option<&ClientFingerprint>,
        site_info: &SiteInfo,
    ) -> Result<TokenInfo, IdentityError>;

    async fn find_by_hash(&mut self, token_hash: &str) -> Result<Option<TokenInfo>, IdentityError>;
    async fn find_by_user(&mut self, user_id: &Uuid) -> Result<Vec<TokenInfo>, IdentityError>;

    async fn delete_token_by_hash(&mut self, kind: TokenKind, token_hash: &str) -> Result<Option<()>, IdentityError>;
    async fn delete_token_by_user(&mut self, user_id: Uuid, token_hash: &str) -> Result<Option<()>, IdentityError>;
    async fn delete_all_token_by_user(&mut self, user_id: Uuid, kinds: &[TokenKind]) -> Result<(), IdentityError>;

    async fn test_token(
        &mut self,
        kind: TokenKind,
        token_hash: &str,
    ) -> Result<Option<(Identity, TokenInfo)>, IdentityError>;

    /// Take a token and return the identity if found.
    /// The token is deleted from the database.
    async fn take_token(
        &mut self,
        kind: TokenKind,
        token_hash: &str,
    ) -> Result<Option<(Identity, TokenInfo)>, IdentityError>;
}

/// Generate a (crypto) hashed version of a token to protect data in rest.
pub fn hash_token(token: &str) -> String {
    // there is no need for a complex hash as key has a big entropy already
    // and it'd be too expensive to invert the hashing.
    let hash = digest::digest(&digest::SHA256, token.as_bytes());
    let hash = hex::encode(hash);
    log::debug!("Hashing token: {token:?} -> [{hash}]");
    hash
}
