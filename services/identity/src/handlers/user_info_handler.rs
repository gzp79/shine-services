use crate::{
    app_state::AppState,
    repositories::{
        identity::{Identity, IdentityDb, IdentityError, TokenKind},
        session::{SessionDb, SessionError},
    },
    services::{events, IdentityService, SessionService},
};
use shine_core::event_bus::EventHandler;
use shine_infra::web::{ClientFingerprint, CurrentUser, Problem, SessionKey, SiteInfo};
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(ThisError, Debug)]
pub enum UserInfoError {
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
    #[error(transparent)]
    SessionError(#[from] SessionError),
}

impl From<UserInfoError> for Problem {
    fn from(value: UserInfoError) -> Self {
        match value {
            UserInfoError::IdentityError(err) => err.into(),
            UserInfoError::SessionError(err) => err.into(),
        }
    }
}

pub struct UserInfo {
    pub identity: Identity,
    pub roles: Vec<String>,
    pub is_linked: bool,
}

pub struct UserInfoHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    identity_service: &'a IdentityService<IDB>,
    session_service: &'a SessionService<SDB>,
}

impl<IDB, SDB> UserInfoHandler<'_, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    pub async fn get_user_info(&self, user_id: Uuid) -> Result<Option<UserInfo>, UserInfoError> {
        // get the version first as newer role is fine, but a deprecated role set is not ok
        // this order ensures the role and other data are at least as fresh as the version

        let identity = match self.identity_service.find_by_id(user_id).await? {
            Some(identity) => identity,
            None => return Ok(None),
        };

        let is_linked = self.identity_service.is_linked(user_id).await?;

        let roles = match self.identity_service.get_roles(user_id).await? {
            Some(roles) => roles,
            None => return Ok(None),
        };

        Ok(Some(UserInfo {
            identity,
            roles,
            is_linked,
        }))
    }

    pub async fn create_user_session(
        &self,
        identity: &Identity,
        fingerprint: &ClientFingerprint,
        site_info: &SiteInfo,
    ) -> Result<Option<CurrentUser>, UserInfoError> {
        let is_linked = self.identity_service.is_linked(identity.id).await?;
        let roles = match self.identity_service.get_roles(identity.id).await? {
            Some(roles) => roles,
            None => return Ok(None),
        };

        // Create session
        log::debug!("Creating session for identity: {:#?}", identity);
        let (user_session, user_session_key) = self
            .session_service
            .create(identity, roles, is_linked, fingerprint, site_info)
            .await?;

        Ok(Some(CurrentUser {
            user_id: user_session.info.user_id,
            key: user_session_key,
            session_start: user_session.info.created_at,
            name: user_session.user.name,
            roles: user_session.user.roles,
            is_email_confirmed: user_session.user.is_email_confirmed,
            is_linked: user_session.user.is_linked,
            fingerprint: user_session.info.fingerprint,
            version: user_session.user_version,
        }))
    }

    pub async fn refresh_user_session(&self, user_id: Uuid) -> Result<(), UserInfoError> {
        match self.get_user_info(user_id).await {
            Ok(Some(user_info)) => {
                // at this point the identity DB has been updated, thus any new session will contain the information
                // not older than the user info just request, thus it should be not an issue if a user signs in
                // during this update process. If there is a frequent update the version should trigger an
                // refresh on the session anyway.
                self.session_service
                    .update_all(&user_info.identity, &user_info.roles, user_info.is_linked)
                    .await?;
                Ok(())
            }
            Ok(None) => {
                log::warn!("User ({}) not found, removing all the sessions", user_id);
                self.session_service.remove_all(user_id).await?;
                Ok(())
            }
            Err(err) => {
                log::warn!("Failed to refresh session for user ({}):", err);
                //self.session_service.remove_all(user_id).await?; - keep sessions, it could be a temporary issue
                Err(err)
            }
        }
    }

    pub async fn revoke_session(&self, user_id: Uuid, session_key: &SessionKey) {
        if let Err(err) = self.session_service.remove(user_id, session_key).await {
            log::error!("Failed to revoke session for user {}: {}", user_id, err);
        }
    }

    pub async fn revoke_access(&self, kind: TokenKind, token: &str) {
        if let Err(err) = self.identity_service.delete_token(kind, token).await {
            log::error!("Failed to revoke ({:?}) token ({}): {}", kind, token, err);
        }
    }
}

impl AppState {
    pub fn user_info_handler(&self) -> UserInfoHandler<impl IdentityDb, impl SessionDb> {
        UserInfoHandler {
            identity_service: self.identity_service(),
            session_service: self.session_service(),
        }
    }

    pub async fn subscribe_user_info_handler(&self) {
        #[derive(Clone)]
        pub struct OnUserUpdate(AppState);
        impl EventHandler<events::UserUpdated> for OnUserUpdate {
            async fn handle(self, event: &events::UserUpdated) {
                let user_id = event.0;

                let handler = self.0.user_info_handler();
                if let Err(err) = handler.refresh_user_session(user_id).await {
                    log::error!(
                        "Failed to refresh session user ({}) after an UserUpdated event: {}",
                        user_id,
                        err
                    );
                }
            }
        }
        self.identity_service().subscribe(OnUserUpdate(self.clone())).await;

        #[derive(Clone)]
        pub struct OnUserRoleUpdate(AppState);
        impl EventHandler<events::UserRoleUpdated> for OnUserRoleUpdate {
            async fn handle(self, event: &events::UserRoleUpdated) {
                let user_id = event.0;

                let handler = self.0.user_info_handler();
                if let Err(err) = handler.refresh_user_session(user_id).await {
                    log::error!(
                        "Failed to refresh session user ({}) after an UserRoleUpdated event: {}",
                        user_id,
                        err
                    );
                }
            }
        }
        self.identity_service().subscribe(OnUserRoleUpdate(self.clone())).await;
    }
}
