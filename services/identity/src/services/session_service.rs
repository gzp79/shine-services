use crate::repositories::{
    identity::Identity,
    session::{Session, SessionDb, SessionError, SessionInfo, Sessions},
};
use chrono::Utc;
use ring::{digest, rand::SystemRandom};
use shine_infra::web::{ClientFingerprint, SessionKey, SiteInfo};
use uuid::Uuid;

pub struct SessionService<DB: SessionDb> {
    db: DB,
    random: SystemRandom,
}

impl<DB> SessionService<DB>
where
    DB: SessionDb,
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
        roles: Vec<String>,
        is_linked: bool,
        fingerprint: &ClientFingerprint,
        site_info: &SiteInfo,
    ) -> Result<(Session, SessionKey), SessionError> {
        let created_at = Utc::now();
        let fingerprint = fingerprint.to_string();
        let session_key = SessionKey::new_random(&self.random)?;
        let session_key_hash = hash_key(&session_key);

        let mut db = self.db.create_context().await?;
        let session = db
            .store_session(
                created_at,
                session_key_hash,
                fingerprint,
                site_info,
                identity,
                roles,
                is_linked,
            )
            .await?;
        Ok((session, session_key))
    }

    #[cfg(test)]
    /// Update the user information of a single session.
    pub async fn update_user_info(
        &self,
        session_key: &SessionKey,
        identity: &Identity,
        roles: &[String],
        is_linked: bool,
    ) -> Result<Option<Session>, SessionError> {
        let session_key_hash = hash_key(session_key);
        let mut db = self.db.create_context().await?;
        db.update_session_user_by_hash(session_key_hash, identity, roles, is_linked)
            .await
    }

    /// Update the user information in all the session of a user
    /// This is not an atomic operation, if new sessions are created they are not touched, but they should
    /// have the new value already.
    pub async fn update_all(&self, identity: &Identity, roles: &[String], is_linked: bool) -> Result<(), SessionError> {
        let mut db = self.db.create_context().await?;

        let key_hashes = db.find_all_session_hashes_by_user(identity.id).await?;
        for key_hash in key_hashes {
            log::debug!("Updating session user info for: {}", key_hash);
            db.update_session_user_by_hash(key_hash, identity, roles, is_linked)
                .await?;
        }

        Ok(())
    }

    /// Get all the active session of the given user.
    pub async fn find_all(&self, user_id: Uuid) -> Result<Vec<SessionInfo>, SessionError> {
        let mut db = self.db.create_context().await?;
        db.find_all_session_infos_by_user(user_id).await
    }

    pub async fn find(&self, user_id: Uuid, session_key: &SessionKey) -> Result<Option<Session>, SessionError> {
        let session_key_hash = hash_key(session_key);
        let mut db = self.db.create_context().await?;
        db.find_session_by_hash(user_id, session_key_hash).await
    }

    /// Remove an active session of the given user.
    pub async fn remove(&self, user_id: Uuid, session_key: &SessionKey) -> Result<(), SessionError> {
        let session_key_hash = hash_key(session_key);
        let mut db = self.db.create_context().await?;
        db.delete_session_by_hash(user_id, session_key_hash).await
    }

    /// Remove all the active session of the given user.
    pub async fn remove_all(&self, user_id: Uuid) -> Result<(), SessionError> {
        let mut db = self.db.create_context().await?;
        db.delete_all_sessions_by_user(user_id).await
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
#[path = "./session_service_test.rs"]
mod session_service_test;
