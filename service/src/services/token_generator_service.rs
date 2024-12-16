use crate::repositories::identity::{IdentityDb, IdentityError, TokenKind};
use chrono::{DateTime, Duration, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use shine_service::{axum::SiteInfo, service::ClientFingerprint};
use thiserror::Error as ThisError;
use uuid::Uuid;

use super::IdentityService;

#[derive(Debug, ThisError)]
pub enum TokenGeneratorError {
    #[error("Failed to generate token: {0}")]
    GeneratorError(String),
    #[error("Retry limit reached")]
    RetryLimitReached,

    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

#[derive(Clone, Debug)]
pub struct UserToken {
    pub user_id: Uuid,
    pub token: String,
    pub token_hash: String,
    pub expire_at: DateTime<Utc>,
}

pub struct TokenGenerator<'a, IDB>
where
    IDB: IdentityDb,
{
    random: &'a SystemRandom,
    identity_service: &'a IdentityService<IDB>,
}

impl<'a, IDB> TokenGenerator<'a, IDB>
where
    IDB: IdentityDb,
{
    pub fn new(random: &'a SystemRandom, identity_service: &'a IdentityService<IDB>) -> Self {
        Self {
            random,
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
        fingerprint: Option<&ClientFingerprint>,
        site_info: &SiteInfo,
    ) -> Result<UserToken, TokenGeneratorError> {
        const MAX_RETRY_COUNT: usize = 10;

        let mut retry_count = 0;
        loop {
            log::debug!("Creating new token for user {user_id}, retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(TokenGeneratorError::RetryLimitReached);
            }
            retry_count += 1;

            let token = self.generate()?;
            match self
                .identity_service
                .add_token(user_id, kind, &token, time_to_live, fingerprint, site_info)
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
