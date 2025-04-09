use crate::repositories::identity::Identity;
use chrono::{DateTime, Utc};
use shine_infra::web::SiteInfo;
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

pub trait Sessions {
    fn store_session(
        &mut self,
        created_at: DateTime<Utc>,
        session_key_hash: String,
        fingerprint: String,
        site_info: &SiteInfo,
        identity: &Identity,
        roles: Vec<String>,
        is_linked: bool,
    ) -> impl Future<Output = Result<Session, SessionError>> + Send;

    fn find_all_session_hashes_by_user(
        &mut self,
        user_id: Uuid,
    ) -> impl Future<Output = Result<Vec<String>, SessionError>> + Send;

    fn find_all_sessions_by_user(
        &mut self,
        user_id: Uuid,
    ) -> impl Future<Output = Result<Vec<Session>, SessionError>> + Send;

    fn find_session_by_hash(
        &mut self,
        user_id: Uuid,
        session_key_hash: &str,
    ) -> impl Future<Output = Result<Option<Session>, SessionError>> + Send;

    fn update_session_user_by_hash(
        &mut self,
        session_key_hash: &str,
        identity: &Identity,
        roles: &[String],
        is_linked: bool,
    ) -> impl Future<Output = Result<Option<Session>, SessionError>> + Send;

    fn delete_session_by_hash(
        &mut self,
        user_id: Uuid,
        session_key_hash: &str,
    ) -> impl Future<Output = Result<(), SessionError>> + Send;

    fn delete_all_sessions_by_user(&mut self, user_id: Uuid) -> impl Future<Output = Result<(), SessionError>> + Send;
}
