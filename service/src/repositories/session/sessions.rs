use crate::repositories::identity::Identity;
use chrono::{DateTime, Utc};
use ring::digest;
use shine_service::{axum::SiteInfo, service::SessionKey};
use std::future::Future;
use uuid::Uuid;

use super::SessionError;

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
    pub roles: Vec<String>,
}

#[derive(Debug)]
pub struct Session {
    pub info: SessionInfo,

    pub user_version: i32,
    pub user: SessionUser,
}

pub trait Sessions {
    fn store_session(
        &mut self,
        created_at: DateTime<Utc>,
        session_key_hash: String,
        fingerprint: String,
        site_info: &SiteInfo,
        identity: &Identity,
        roles: Vec<String>,
    ) -> impl Future<Output = Result<Session, SessionError>> + Send;

    fn find_all_session_hashes_by_user(
        &mut self,
        user_id: Uuid,
    ) -> impl Future<Output = Result<Vec<String>, SessionError>> + Send;

    fn find_all_session_infos_by_user(
        &mut self,
        user_id: Uuid,
    ) -> impl Future<Output = Result<Vec<SessionInfo>, SessionError>> + Send;

    fn find_session_by_hash(
        &mut self,
        user_id: Uuid,
        session_key_hash: String,
    ) -> impl Future<Output = Result<Option<Session>, SessionError>> + Send;

    fn update_session_user_by_hash(
        &mut self,
        session_key_hash: String,
        identity: &Identity,
        roles: &[String],
    ) -> impl Future<Output = Result<Option<Session>, SessionError>> + Send;

    fn delete_session_by_hash(
        &mut self,
        user_id: Uuid,
        session_key_hash: String,
    ) -> impl Future<Output = Result<(), SessionError>> + Send;

    fn delete_all_sessions_by_user(&mut self, user_id: Uuid) -> impl Future<Output = Result<(), SessionError>> + Send;
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
