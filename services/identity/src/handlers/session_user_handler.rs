use crate::{
    app_state::AppState,
    repositories::{
        identity::{Identity, IdentityDb, IdentityError, TokenKind},
        session::{SessionDb, SessionError},
    },
    services::{IdentityService, SessionService},
};
use shine_core::web::{CurrentUser, Problem};
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(ThisError, Debug)]
pub enum SessionUserSyncError {
    #[error("User not found: {0}")]
    UserNotFound(Uuid),
    #[error("User role not found: {0}")]
    RolesNotFound(Uuid),

    #[error(transparent)]
    IdentityError(#[from] IdentityError),
    #[error(transparent)]
    SessionError(#[from] SessionError),
}

impl From<SessionUserSyncError> for Problem {
    fn from(value: SessionUserSyncError) -> Self {
        match value {
            SessionUserSyncError::UserNotFound(user_id) => {
                Problem::not_found().with_instance_str(format!("{{identity_api}}/identities/{}", user_id))
            }
            SessionUserSyncError::RolesNotFound(user_id) => {
                Problem::not_found().with_instance_str(format!("{{identity_api}}/identities/{}", user_id))
            }
            SessionUserSyncError::IdentityError(err) => err.into(),
            SessionUserSyncError::SessionError(err) => err.into(),
        }
    }
}

pub struct SessionUserHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    identity_service: &'a IdentityService<IDB>,
    session_service: &'a SessionService<SDB>,
}

impl<'a, IDB, SDB> SessionUserHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    async fn get_user_info(&self, user_id: Uuid) -> Result<(Identity, Vec<String>), SessionUserSyncError> {
        // get the version first as newer role is fine, but a deprecated role set is not ok
        // this order ensures the role and other data are at least as fresh as the version
        let identity = self
            .identity_service
            .find_by_id(user_id)
            .await?
            .ok_or(SessionUserSyncError::UserNotFound(user_id))?;

        let roles = self
            .identity_service
            .get_roles(user_id)
            .await?
            .ok_or(SessionUserSyncError::RolesNotFound(user_id))?;

        Ok((identity, roles))
    }

    pub async fn refresh_session_user(&self, user_id: Uuid) -> Result<(Identity, Vec<String>), SessionUserSyncError> {
        match self.get_user_info(user_id).await {
            Ok((identity, roles)) => {
                // at this point the DB has been updated, thus any new session will contain the information
                // not older than the queried user info, thus it should be not an issue if a users sign in
                // during this update process.
                self.session_service.update_all(&identity, &roles).await?;
                Ok((identity, roles))
            }
            Err(err) => {
                self.session_service.remove_all(user_id).await?;
                Err(err)
            }
        }
    }

    pub async fn revoke_session(&self, user_session: CurrentUser) {
        if let Err(err) = self
            .session_service
            .remove(user_session.user_id, &user_session.key)
            .await
        {
            log::error!("Failed to revoke session for user {}: {}", user_session.user_id, err);
        }
    }

    pub async fn revoke_opt_session(&self, user_session: Option<CurrentUser>) {
        if let Some(user_session) = user_session {
            self.revoke_session(user_session).await;
        }
    }

    pub async fn revoke_access(&self, kind: TokenKind, token: &str) {
        if let Err(err) = self.identity_service.delete_token(kind, token).await {
            log::error!("Failed to revoke ({:?}) token ({}): {}", kind, token, err);
        }
    }

    pub async fn revoke_opt_access(&self, kind: TokenKind, token: Option<String>) {
        if let Some(revoked_token) = token {
            self.revoke_access(kind, &revoked_token).await;
        }
    }
}

impl AppState {
    pub fn session_user_handler(&self) -> SessionUserHandler<impl IdentityDb, impl SessionDb> {
        SessionUserHandler {
            identity_service: self.identity_service(),
            session_service: self.session_service(),
        }
    }
}
