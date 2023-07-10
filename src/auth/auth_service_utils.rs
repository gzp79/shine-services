use crate::{
    auth::{auth_session::TokenLogin, AuthServiceState},
    db::{ExternalLoginInfo, Identity, IdentityError, NameGeneratorError},
};
use chrono::Duration;
use ring::rand::SecureRandom;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum UserCreateError {
    #[error("Retry limit reach for user creation")]
    RetryLimitReached,
    #[error(transparent)]
    NameGeneratorError(#[from] NameGeneratorError),
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl AuthServiceState {
    pub(in crate::auth) async fn create_user_with_retry(
        &self,
        mut default_name: Option<&str>,
        email: Option<&str>,
        external_login: Option<&ExternalLoginInfo>,
    ) -> Result<Identity, UserCreateError> {
        const MAX_RETRY_COUNT: usize = 10;
        let mut retry_count = 0;
        loop {
            log::debug!("Creating new user; retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(UserCreateError::RetryLimitReached);
            }
            retry_count += 1;

            let user_id = Uuid::new_v4();
            let user_name = match default_name.take() {
                Some(name) => name.to_string(),
                None => self.name_generator().generate_name().await?,
            };

            match self
                .identity_manager()
                .create_user(user_id, &user_name, email, external_login)
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

#[derive(Debug, ThisError)]
pub enum TokenCreateError {
    #[error("Retry limit reach for token creation")]
    RetryLimitReached,
    #[error("Failed to generate token: {0}")]
    TokenGen(String),
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl AuthServiceState {
    fn generate_token(random: &(dyn SecureRandom + Send + Sync)) -> Result<String, TokenCreateError> {
        let mut raw = [0_u8; 16];
        random
            .fill(&mut raw)
            .map_err(|err| TokenCreateError::TokenGen(format!("{err:#?}")))?;
        Ok(hex::encode(raw))
    }

    // Create a new login token for the given user.
    pub(in crate::auth) async fn create_token_with_retry(
        &self,
        user_id: Uuid,
        token_max_duration: Duration,
        random: &(dyn SecureRandom + Send + Sync),
    ) -> Result<TokenLogin, TokenCreateError> {
        const MAX_RETRY_COUNT: usize = 10;
        let mut retry_count = 0;
        loop {
            log::debug!("Creating new token for user {user_id}, retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(TokenCreateError::RetryLimitReached);
            }
            retry_count += 1;

            let token = Self::generate_token(random)?;
            match self
                .identity_manager()
                .create_token(user_id, &token, &token_max_duration)
                .await
            {
                Ok(token) => {
                    return Ok(TokenLogin {
                        token: token.token,
                        expires: token.expire_at,
                    })
                }
                Err(IdentityError::TokenConflict) => continue,
                Err(err) => return Err(TokenCreateError::IdentityError(err)),
            }
        }
    }
}
