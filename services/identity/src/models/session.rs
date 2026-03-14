use chrono::{DateTime, Utc};
use shine_infra::web::extracts::SiteInfo;
use uuid::Uuid;

/// The immutable part of the session information.
#[derive(Debug)]
pub struct SessionInfo {
    pub created_at: DateTime<Utc>,
    pub user_id: Uuid,
    pub key_hash: String,
    pub fingerprint: String,
    pub site_info: SiteInfo,
}

/// The user part of the session information.
#[derive(Debug)]
pub struct SessionUser {
    pub name: String,
    pub is_linked: bool,
    pub is_email_confirmed: bool,
    pub roles: Vec<String>,
}

#[derive(Debug)]
pub struct Session {
    pub info: SessionInfo,
    pub user: SessionUser,
    pub expire_at: DateTime<Utc>,
}
