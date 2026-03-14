use chrono::{DateTime, Utc};
use shine_infra::{models::Email, web::extracts::ClientFingerprint};
use uuid::Uuid;

use super::TokenKind;

#[derive(Debug)]
pub struct TokenInfo {
    pub user_id: Uuid,
    pub kind: TokenKind,
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub expire_at: DateTime<Utc>,
    pub is_expired: bool,
    pub bound_fingerprint: Option<String>,
    pub bound_email: Option<Email>,
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
