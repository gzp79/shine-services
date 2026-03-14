use crate::models::{Identity, IdentityError, TokenInfo, TokenKind};
use chrono::Duration;
use shine_infra::web::extracts::{ClientFingerprint, SiteInfo};
use std::future::Future;
use uuid::Uuid;

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
