use crate::{
    app_state::AppState,
    models::{Identity, IdentityError, SessionError},
    repositories::{
        identity::{pg::PgIdentityDb, IdentityDb},
        session::{redis::RedisSessionDb, SessionDb},
    },
    services::{LinkService, RoleService, SessionService, UserService},
};
use shine_infra::{
    session::CurrentUser,
    web::{
        extracts::{ClientFingerprint, SiteInfo},
        responses::Problem,
    },
};
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

pub struct UserSessionHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    user_service: &'a UserService<IDB>,
    link_service: &'a LinkService<IDB>,
    role_service: &'a RoleService<IDB>,
    session_service: &'a SessionService<SDB>,
}

impl<'a, IDB, SDB> UserSessionHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    pub fn new(
        user_service: &'a UserService<IDB>,
        link_service: &'a LinkService<IDB>,
        role_service: &'a RoleService<IDB>,
        session_service: &'a SessionService<SDB>,
    ) -> Self {
        Self {
            user_service,
            link_service,
            role_service,
            session_service,
        }
    }

    pub async fn get_user_info(&self, user_id: Uuid) -> Result<Option<UserInfo>, UserInfoError> {
        // get the version first as newer role is fine, but a deprecated role set is not ok
        // this order ensures the role and other data are at least as fresh as the version

        let identity = match self.user_service.find_by_id(user_id).await? {
            Some(identity) => identity,
            None => return Ok(None),
        };

        let is_linked = self.link_service.is_linked(user_id).await?;

        let roles = match self.role_service.get_roles(user_id).await? {
            Some(roles) => roles,
            None => return Ok(None),
        };

        Ok(Some(UserInfo { identity, roles, is_linked }))
    }

    pub async fn create_user_session(
        &self,
        identity: &Identity,
        fingerprint: &ClientFingerprint,
        site_info: &SiteInfo,
    ) -> Result<Option<CurrentUser>, UserInfoError> {
        let is_linked = self.link_service.is_linked(identity.id).await?;
        let roles = match self.role_service.get_roles(identity.id).await? {
            Some(roles) => roles,
            None => return Ok(None),
        };

        // Create session
        log::debug!("Creating session for identity: {identity:#?}");
        let (user_session, user_session_key) = self
            .session_service
            .create(identity, roles, is_linked, fingerprint, site_info)
            .await?;

        Ok(Some(CurrentUser {
            user_id: user_session.info.user_id,
            key: user_session_key,
            session_start: user_session.info.created_at,
            session_end: user_session.expire_at,
            name: user_session.user.name,
            roles: user_session.user.roles,
            is_email_confirmed: user_session.user.is_email_confirmed,
            is_linked: user_session.user.is_linked,
            fingerprint: user_session.info.fingerprint,
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
                log::warn!("User ({user_id}) not found, removing all the sessions");
                self.session_service.remove_all(user_id).await?;
                Ok(())
            }
            Err(err) => {
                log::warn!("Failed to refresh session for user ({err}):");
                //self.session_service.remove_all(user_id).await?; - keep sessions, it could be a temporary issue
                Err(err)
            }
        }
    }
}

impl AppState {
    pub fn user_session_handler(&self) -> UserSessionHandler<'_, PgIdentityDb, RedisSessionDb> {
        UserSessionHandler::new(
            self.user_service(),
            self.link_service(),
            self.role_service(),
            self.session_service(),
        )
    }
}
