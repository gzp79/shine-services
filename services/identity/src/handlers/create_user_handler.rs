use crate::{
    app_state::AppState,
    repositories::identity::{ExternalUserInfo, Identity, IdentityDb, IdentityError},
    services::{LinkService, UserService},
};
use shine_infra::web::responses::Problem;
use thiserror::Error as ThisError;
use uuid::Uuid;
use validator::ValidateEmail;

#[derive(Debug, ThisError)]
pub enum CreateUserError {
    #[error("Retry limit reach for user creation")]
    RetryLimitReached,
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl From<CreateUserError> for Problem {
    fn from(err: CreateUserError) -> Self {
        match err {
            CreateUserError::IdentityError(err) => err.into(),

            err => Problem::internal_error().with_detail(err.to_string()),
        }
    }
}

pub struct CreateUserHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    user_service: &'a UserService<IDB>,
    link_service: &'a LinkService<IDB>,
}

impl<'a, IDB> CreateUserHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    pub fn new(user_service: &'a UserService<IDB>, link_service: &'a LinkService<IDB>) -> Self {
        Self { user_service, link_service }
    }

    pub async fn create_user(
        &self,
        external_user: Option<&ExternalUserInfo>,
        name: Option<&str>,
        email: Option<&str>,
    ) -> Result<Identity, CreateUserError> {
        const MAX_RETRY_COUNT: usize = 10;

        let mut name = name.map(|e| e.to_owned());
        let email = email.filter(|email| email.validate_email()).map(|email| (email, false));

        assert!(email.as_ref().is_none_or(|(email, _)| email.validate_email()));

        let mut retry_count = 0;
        loop {
            log::debug!("Creating new user; retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(CreateUserError::RetryLimitReached);
            }
            retry_count += 1;

            let user_id = Uuid::new_v4();
            let user_name = match name.take() {
                Some(name) => name,
                None => self.user_service.generate_name().await?,
            };

            // Create user with or without external link
            let result = if let Some(ext_user) = external_user {
                self.user_service
                    .create_linked_user(user_id, &user_name, ext_user)
                    .await
            } else {
                self.user_service.create(user_id, &user_name, email).await
            };

            match result {
                Ok(identity) => return Ok(identity),
                Err(IdentityError::NameConflict) => continue,
                Err(IdentityError::UserIdConflict) => continue,
                Err(err) => return Err(CreateUserError::IdentityError(err)),
            }
        }
    }
}

impl AppState {
    pub fn create_user_service(&self) -> CreateUserHandler<impl IdentityDb> {
        CreateUserHandler::new(self.user_service(), self.link_service())
    }
}
