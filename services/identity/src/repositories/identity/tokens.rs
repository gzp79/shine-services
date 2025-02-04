use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shine_core::web::{ClientFingerprint, SiteInfo};
use std::future::Future;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{Identity, IdentityError};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
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
    fn store_token(
        &mut self,
        user_id: Uuid,
        kind: TokenKind,
        token_hash: &str,
        time_to_live: &Duration,
        fingerprint: Option<&ClientFingerprint>,
        site_info: &SiteInfo,
    ) -> impl Future<Output = Result<TokenInfo, IdentityError>> + Send;

    fn find_by_hash(
        &mut self,
        token_hash: &str,
    ) -> impl Future<Output = Result<Option<TokenInfo>, IdentityError>> + Send;

    fn find_by_user(&mut self, user_id: &Uuid) -> impl Future<Output = Result<Vec<TokenInfo>, IdentityError>> + Send;

    fn delete_token_by_hash(
        &mut self,
        kind: TokenKind,
        token_hash: &str,
    ) -> impl Future<Output = Result<Option<()>, IdentityError>> + Send;

    fn delete_token_by_user(
        &mut self,
        user_id: Uuid,
        token_hash: &str,
    ) -> impl Future<Output = Result<Option<()>, IdentityError>> + Send;

    fn delete_all_token_by_user(
        &mut self,
        user_id: Uuid,
        kinds: &[TokenKind],
    ) -> impl Future<Output = Result<(), IdentityError>> + Send;

    fn test_token(
        &mut self,
        kind: TokenKind,
        token_hash: &str,
    ) -> impl Future<Output = Result<Option<(Identity, TokenInfo)>, IdentityError>> + Send;

    /// Take a token and return the identity if found.
    /// The token is deleted from the database.
    fn take_token(
        &mut self,
        kind: TokenKind,
        token_hash: &str,
    ) -> impl Future<Output = Result<Option<(Identity, TokenInfo)>, IdentityError>> + Send;
}
