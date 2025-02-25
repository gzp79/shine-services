use crate::{
    repositories::identity::{IdentityDb, IdentityError, TokenKind},
    services::{IdentityService, SettingsService},
};
use chrono::{DateTime, Duration, Utc};
use ring::{
    hmac,
    rand::{SecureRandom, SystemRandom},
};
use shine_core::web::{ClientFingerprint, Problem, SiteInfo};
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum TokenGeneratorError {
    #[error("Failed to generate token: {0}")]
    GeneratorError(String),
    #[error("Retry limit reached")]
    RetryLimitReached,

    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl From<TokenGeneratorError> for Problem {
    fn from(err: TokenGeneratorError) -> Self {
        match err {
            TokenGeneratorError::IdentityError(err) => err.into(),

            err => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}

#[derive(Clone, Debug)]
pub struct UserToken {
    pub user_id: Uuid,
    pub token: String,
    pub token_hash: String,
    pub expire_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct EmailToken {
    pub user_id: Uuid,
    pub token: String,
    pub expire_at: DateTime<Utc>,
    pub current_email: Option<String>,
    pub new_email: String,
}

pub struct TokenGenerator<'a, IDB>
where
    IDB: IdentityDb,
{
    random: &'a SystemRandom,
    settings: &'a SettingsService,
    identity_service: &'a IdentityService<IDB>,
}

impl<'a, IDB> TokenGenerator<'a, IDB>
where
    IDB: IdentityDb,
{
    pub fn new(
        random: &'a SystemRandom,
        settings: &'a SettingsService,
        identity_service: &'a IdentityService<IDB>,
    ) -> Self {
        Self {
            random,
            settings,
            identity_service,
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

    pub fn generate_email_verify_token(
        &self,
        user_id: Uuid,
        current_email: Option<&str>,
        new_email: &str,
        expire_at: &DateTime<Utc>,
    ) -> String {
        let msg = format!("{}{}{}{}", user_id, current_email.unwrap_or(""), new_email, expire_at);
        let token = hmac::sign(&self.settings.token.email_key, msg.as_bytes());
        let token_hex = hex::encode(token.as_ref());
        let token = format!("{};{:x}", token_hex, expire_at.timestamp());
        token
    }

    pub async fn create_email_verify_token(
        &self,
        user_id: Uuid,
        time_to_live: &Duration,
        new_email: Option<String>,
    ) -> Result<EmailToken, TokenGeneratorError> {
        let email = self
            .identity_service
            .find_by_id(user_id)
            .await?
            .ok_or(IdentityError::UserDeleted { id: user_id })?
            .email;

        let new_email = new_email.or(email.clone()).ok_or(IdentityError::MissingEmail)?;
        let expire_at = Utc::now() + *time_to_live;
        let token = self.generate_email_verify_token(user_id, email.as_deref(), &new_email, &expire_at);

        Ok(EmailToken {
            user_id,
            token: token,
            expire_at: expire_at,
            current_email: email,
            new_email,
        })
    }
}

#[cfg(test)]
mod test {
    use axum_extra::extract::cookie;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
    use ring::{digest, rand};
    use shine_test::test;

    #[test]
    #[ignore = "This is not a test but a helper to generate secret"]
    fn generate_cookie_secret() {
        let key = cookie::Key::generate();
        println!("{}", B64.encode(key.master()));
    }

    #[test]
    #[ignore = "This is not a test but a helper to generate secret"]
    fn generate_email_token_secret() {
        let rng = rand::SystemRandom::new();
        let key: [u8; digest::SHA256_OUTPUT_LEN] = rand::generate(&rng).unwrap().expose();
        println!("{}", B64.encode(key));
    }
}
