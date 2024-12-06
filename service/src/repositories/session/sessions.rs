use crate::repositories::{Identity, Role};
use chrono::{DateTime, Utc};
use ring::digest;
use shine_service::{
    axum::SiteInfo,
    service::{ClientFingerprint, SessionKey},
};
use uuid::Uuid;

use super::session_error::SessionError;

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
    pub is_email_confirmed: bool,
    pub roles: Vec<Role>,
}

#[derive(Debug)]
pub struct Session {
    pub info: SessionInfo,

    pub user_version: i32,
    pub user: SessionUser,
}

pub trait Sessions {
    async fn store_session(
        &mut self,
        created_at: DateTime<Utc>,
        session_key_hash: String,
        fingerprint: String,
        site_info: &SiteInfo,
        identity: &Identity,
        roles: Vec<Role>,
    ) -> Result<Session, SessionError>;

    async fn find_all_session_hashes_by_user(&mut self, user_id: Uuid) -> Result<Vec<String>, SessionError>;
    async fn find_all_session_infos_by_user(&mut self, user_id: Uuid) -> Result<Vec<SessionInfo>, SessionError>;

    async fn find_session_by_hash(
        &mut self,
        user_id: Uuid,
        session_key_hash: String,
    ) -> Result<Option<Session>, SessionError>;

    async fn update_session_user_by_hash(
        &mut self,
        session_key_hash: String,
        identity: &Identity,
        roles: &[Role],
    ) -> Result<Option<Session>, SessionError>;

    async fn delete_session_by_hash(&mut self, user_id: Uuid, session_key_hash: String) -> Result<(), SessionError>;
    async fn delete_all_sessions_by_user(&mut self, user_id: Uuid) -> Result<(), SessionError>;
}

/// Generate a (crypto) hashed version of a session key to protect data in rest.
pub fn hash_key(key: &SessionKey) -> String {
    // there is no need for a complex hash as key has a big entropy already
    // and it'd be too expensive to invert the hashing.
    let hash = digest::digest(&digest::SHA256, key.as_bytes());
    let hash = hex::encode(hash);
    log::debug!("Hashing session key: {key:?} -> [{hash}]");
    hash
}
