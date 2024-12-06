use crate::repositories::{Identity, Role};
use chrono::{DateTime, Utc};
use ring::{digest, rand::SystemRandom};
use shine_service::{
    axum::SiteInfo,
    service::{ClientFingerprint, RedisConnectionPool, SessionKey},
};
use uuid::Uuid;

use super::{
    session_db::{SessionDb, SessionDbContext},
    session_error::SessionError,
    sessions::{Session, SessionInfo, Sessions},
};

#[derive(Debug)]
pub struct SessionSentinel {
    pub created_at: DateTime<Utc>,
    pub fingerprint: String,
    pub agent: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

#[derive(Debug)]
struct SessionData {
    pub name: String,
    pub is_email_confirmed: bool,
    pub roles: Vec<Role>,
}

pub struct Inner {
    redis: RedisConnectionPool,
    key_prefix: String,
    ttl_session: i64,
    random: SystemRandom,
}

#[derive(Clone)]
pub struct SessionManager<DB: SessionDb + Clone> {
    db: DB,
    random: SystemRandom,
}

impl<DB> SessionManager<DB>
where
    DB: SessionDb + Clone,
{
    pub fn new(db: DB) -> Self {
        Self {
            db,
            random: SystemRandom::new(),
        }
    }

    /// Creates a new session for the given user. This is the only place where the generated raw session key is accessible in the server,
    /// all the other raw keys are provided by the client and are not stored.
    pub async fn create(
        &self,
        identity: &Identity,
        roles: Vec<Role>,
        fingerprint: &ClientFingerprint,
        site_info: &SiteInfo,
    ) -> Result<(Session, SessionKey), SessionError> {
        let mut db: <DB as SessionDb>::Context<'_> = self.db.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let created_at = Utc::now();
        let fingerprint = fingerprint.to_string();
        let session_key = SessionKey::new_random(&self.random)?;
        let session_key_hash = hash_key(&session_key);

        let session = transaction
            .store_session(created_at, session_key_hash, fingerprint, site_info, identity, roles)
            .await?;
        Ok((session, session_key))
    }

    /// Update the user information in a session.
    pub async fn update_user_info(
        &self,
        session_key: &SessionKey,
        identity: &Identity,
        roles: &[Role],
    ) -> Result<Option<Session>, SessionError> {
        let mut db: <DB as SessionDb>::Context<'_> = self.db.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let session_key_hash = hash_key(&session_key);

        transaction
            .update_session_user_by_hash(session_key_hash, identity, roles)
            .await
    }

    /// Update the user information in all the session of a user
    /// This is not an atomic operation, if new sessions are created they are not touched, but they should
    /// have the new value already.
    pub async fn update_all(&self, identity: &Identity, roles: &[Role]) -> Result<(), SessionError> {
        let mut db: <DB as SessionDb>::Context<'_> = self.db.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let key_hashes = transaction.find_all_session_hashes_by_user(identity.id).await?;

        for key_hash in key_hashes {
            transaction
                .update_session_user_by_hash(key_hash, identity, roles)
                .await?;
        }

        Ok(())
    }

    /// Get all the active session of the given user.
    pub async fn find_all(&self, user_id: Uuid) -> Result<Vec<SessionInfo>, SessionError> {
        let mut db: <DB as SessionDb>::Context<'_> = self.db.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.find_all_session_infos_by_user(user_id).await
    }

    pub async fn find(&self, user_id: Uuid, session_key: &SessionKey) -> Result<Option<Session>, SessionError> {
        let mut db: <DB as SessionDb>::Context<'_> = self.db.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let session_key_hash = hash_key(&session_key);

        transaction.find_session_by_hash(user_id, session_key_hash).await
    }

    /// Remove an active session of the given user.
    pub async fn remove(&self, user_id: Uuid, session_key: &SessionKey) -> Result<(), SessionError> {
        let mut db: <DB as SessionDb>::Context<'_> = self.db.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        let session_key_hash = hash_key(&session_key);

        transaction.delete_session_by_hash(user_id, session_key_hash).await
    }

    /// Remove all the active session of the given user.
    pub async fn remove_all(&self, user_id: Uuid) -> Result<(), SessionError> {
        let mut db: <DB as SessionDb>::Context<'_> = self.db.create_context().await?;
        let mut transaction = db.begin_transaction().await?;

        transaction.delete_all_sessions_by_user(user_id).await
    }
}

/// Generate a (crypto) hashed version of a session key to protect data in rest.
fn hash_key(key: &SessionKey) -> String {
    // there is no need for a complex hash as key has a big entropy already
    // and it'd be too expensive to invert the hashing.
    let hash = digest::digest(&digest::SHA256, key.as_bytes());
    let hash = hex::encode(hash);
    log::debug!("Hashing session key: {key:?} -> [{hash}]");
    hash
}

#[cfg(test)]
#[path = "./session_manager_test.rs"]
mod session_manager_test;
