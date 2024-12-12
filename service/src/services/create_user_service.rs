use crate::repositories::{
    identity::{ExternalUserInfo, Identity, IdentityDb, IdentityError},
    session::SessionDb,
};
use thiserror::Error as ThisError;
use uuid::Uuid;

use super::{IdentityService, SessionService};

#[derive(Debug, ThisError)]
pub enum UserCreateError {
    #[error("Retry limit reach for user creation")]
    RetryLimitReached,
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

pub struct CreateUserService<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    identity_service: &'a IdentityService<IDB>,
    session_service: &'a SessionService<SDB>,
}

impl<'a, IDB, SDB> CreateUserService<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    pub fn new(identity_service: &'a IdentityService<IDB>, session_service: &'a SessionService<SDB>) -> Self {
        Self {
            identity_service,
            session_service,
        }
    }

    pub async fn create_user(&self, external_user: Option<&ExternalUserInfo>) -> Result<Identity, UserCreateError> {
        const MAX_RETRY_COUNT: usize = 10;

        let mut default_name = external_user.as_ref().and_then(|u| u.name.clone());
        let email = external_user.as_ref().and_then(|u| u.email.as_deref());
        let mut retry_count = 0;
        loop {
            log::debug!("Creating new user; retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(UserCreateError::RetryLimitReached);
            }
            retry_count += 1;

            let user_id = Uuid::new_v4();
            let user_name = match default_name.take() {
                Some(name) => name,
                None => self.identity_service.generate_user_name().await?,
            };

            match self
                .identity_service
                .create_user(user_id, &user_name, email, external_user)
                .await
            {
                Ok(identity) => return Ok(identity),
                Err(IdentityError::NameConflict) => continue,
                Err(IdentityError::UserIdConflict) => continue,
                Err(err) => return Err(UserCreateError::IdentityError(err)),
            }
        }
    }
}
