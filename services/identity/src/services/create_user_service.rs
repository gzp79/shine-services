use crate::{
    repositories::identity::{ExternalUserInfo, Identity, IdentityDb, IdentityError},
    services::IdentityService,
};
use shine_core::web::Problem;
use thiserror::Error as ThisError;
use uuid::Uuid;
use validator::ValidateEmail;

#[derive(Debug, ThisError)]
pub enum UserCreateError {
    #[error("Retry limit reach for user creation")]
    RetryLimitReached,
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl From<UserCreateError> for Problem {
    fn from(err: UserCreateError) -> Self {
        match err {
            UserCreateError::IdentityError(err) => err.into(),

            err => Problem::internal_error().with_detail(err.to_string()),
        }
    }
}

pub struct CreateUserService<'a, IDB>
where
    IDB: IdentityDb,
{
    identity_service: &'a IdentityService<IDB>,
}

impl<'a, IDB> CreateUserService<'a, IDB>
where
    IDB: IdentityDb,
{
    pub fn new(identity_service: &'a IdentityService<IDB>) -> Self {
        Self { identity_service }
    }

    pub async fn create_user(
        &self,
        external_user: Option<&ExternalUserInfo>,
        confirmed_email: Option<&str>,
    ) -> Result<Identity, UserCreateError> {
        const MAX_RETRY_COUNT: usize = 10;

        let mut default_name = external_user.as_ref().and_then(|u| u.name.clone());
        let email = match confirmed_email {
            Some(email) => Some((email, true)),
            None => external_user
                .as_ref()
                .and_then(|u| u.email.as_deref())
                .filter(|email| email.validate_email())
                .map(|email| (email, false)),
        };

        assert!(email.as_ref().map_or(true, |(email, _)| email.validate_email()));

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
