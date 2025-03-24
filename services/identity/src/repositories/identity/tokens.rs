use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shine_infra::web::{ClientFingerprint, SiteInfo};
use std::future::Future;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{Identity, IdentityError};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum TokenKind {
    Access,
    Persistent,
    SingleAccess,
    EmailAccess,
}

impl TokenKind {
    // return if token can be used only once
    pub fn is_single_access(&self) -> bool {
        matches!(self, Self::SingleAccess) || matches!(self, Self::EmailAccess)
    }

    pub fn all() -> &'static [TokenKind] {
        &[
            TokenKind::Access,
            TokenKind::Persistent,
            TokenKind::SingleAccess,
            TokenKind::EmailAccess,
        ]
    }

    pub fn all_single_access() -> &'static [TokenKind] {
        &[TokenKind::SingleAccess, TokenKind::EmailAccess]
    }

    pub fn all_multi_access() -> &'static [TokenKind] {
        &[TokenKind::Access, TokenKind::Persistent]
    }
}

#[derive(Debug)]
pub struct TokenInfo {
    pub user_id: Uuid,
    pub kind: TokenKind,
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub expire_at: DateTime<Utc>,
    pub is_expired: bool,
    pub bound_fingerprint: Option<String>,
    pub bound_email: Option<String>,
    pub agent: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

impl TokenInfo {
    pub fn check_fingerprint(&self, fingerprint: &ClientFingerprint) -> bool {
        self.bound_fingerprint.is_none() || Some(fingerprint.as_str()) == self.bound_fingerprint.as_deref()
    }
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
        email: Option<&str>,
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

    /// Test if the token is valid and return the identity if found.
    /// It can be used only for tokens that can be used multiple times. (is_single_access() == false)
    fn test_token(
        &mut self,
        allowed_kind: &[TokenKind],
        token_hash: &str,
    ) -> impl Future<Output = Result<Option<(Identity, TokenInfo)>, IdentityError>> + Send;

    /// Take a token and return the identity if found.
    /// The token is deleted from the database, thus it is used mainly for single access tokens. (is_single_access() == true)
    fn take_token(
        &mut self,
        allowed_kind: &[TokenKind],
        token_hash: &str,
    ) -> impl Future<Output = Result<Option<(Identity, TokenInfo)>, IdentityError>> + Send;
}
