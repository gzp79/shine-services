use crate::repositories::{
    identity::{IdentityDb, IdentityError, TokenKind},
    mailer::{EmailSender, EmailSenderError},
};
use chrono::{DateTime, Duration, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use shine_core::web::{ClientFingerprint, SiteInfo};
use thiserror::Error as ThisError;
use uuid::Uuid;

use super::{IdentityService, MailerService};

#[derive(Debug, ThisError)]
pub enum TokenGeneratorError {
    #[error("Failed to generate token: {0}")]
    GeneratorError(String),
    #[error("Retry limit reached")]
    RetryLimitReached,
    #[error("User has no email address")]
    MissingEmailAddress,

    #[error(transparent)]
    IdentityError(#[from] IdentityError),
    #[error(transparent)]
    MailerError(#[from] EmailSenderError),
}

#[derive(Clone, Debug)]
pub struct UserToken {
    pub user_id: Uuid,
    pub token: String,
    pub token_hash: String,
    pub expire_at: DateTime<Utc>,
}

pub struct TokenGenerator<'a, IDB, ES>
where
    IDB: IdentityDb,
    ES: EmailSender,
{
    random: &'a SystemRandom,
    identity_service: &'a IdentityService<IDB>,
    mailer_service: &'a MailerService<ES>,
}

impl<'a, IDB, ES> TokenGenerator<'a, IDB, ES>
where
    IDB: IdentityDb,
    ES: EmailSender,
{
    pub fn new(
        random: &'a SystemRandom,
        identity_service: &'a IdentityService<IDB>,
        mailer_service: &'a MailerService<ES>,
    ) -> Self {
        Self {
            random,
            identity_service,
            mailer_service,
        }
    }

    pub fn generate(&self) -> Result<String, TokenGeneratorError> {
        let mut raw = [0_u8; 16];
        //todo: is it cryptographically secure?
        self.random
            .fill(&mut raw)
            .map_err(|err| TokenGeneratorError::GeneratorError(format!("{err:#?}")))?;
        Ok(hex::encode(raw))
    }

    pub async fn create_user_token(
        &self,
        user_id: Uuid,
        kind: TokenKind,
        time_to_live: &Duration,
        fingerprint_to_bind_to: Option<&ClientFingerprint>,
        email_to_bind_to: Option<&str>,
        site_info: &SiteInfo,
    ) -> Result<UserToken, TokenGeneratorError> {
        const MAX_RETRY_COUNT: usize = 10;

        let mut retry_count = 0;
        loop {
            log::debug!("Creating new {kind:?} token for user {user_id}, retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(TokenGeneratorError::RetryLimitReached);
            }
            retry_count += 1;

            let token = self.generate()?;
            match self
                .identity_service
                .add_token(
                    user_id,
                    kind,
                    &token,
                    time_to_live,
                    fingerprint_to_bind_to,
                    email_to_bind_to,
                    site_info,
                )
                .await
            {
                Ok(info) => {
                    return Ok(UserToken {
                        user_id,
                        token,
                        token_hash: info.token_hash,
                        expire_at: info.expire_at,
                    })
                }
                Err(IdentityError::TokenConflict) => continue,
                Err(err) => return Err(TokenGeneratorError::IdentityError(err)),
            }
        }
    }

    pub async fn create_email_token(
        &self,
        user_id: Uuid,
        time_to_live: &Duration,
        site_info: &SiteInfo,
    ) -> Result<(), TokenGeneratorError> {
        let kind = TokenKind::EmailVerify;
        let email = self
            .identity_service
            .find_by_id(user_id)
            .await
            .map_err(TokenGeneratorError::IdentityError)?
            .ok_or(IdentityError::UserDeleted)?
            .email
            .ok_or(TokenGeneratorError::MissingEmailAddress)?;
        let token = self
            .create_user_token(user_id, kind, time_to_live, None, Some(&email), site_info)
            .await?;
        self.mailer_service
            .send_confirmation_email(&email, &token.token)
            .await
            .map_err(TokenGeneratorError::MailerError)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use axum_extra::extract::cookie::Key;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
    use shine_test::test;

    #[test]
    #[ignore = "This is not a test but a helper to generate secret"]
    fn generate_cookie_secret() {
        let key = Key::generate();
        println!("{}", B64.encode(key.master()));
    }
}
