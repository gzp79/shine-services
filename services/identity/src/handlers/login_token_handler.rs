use crate::{
    app_state::AppState,
    repositories::identity::{IdentityDb, IdentityError, TokenKind},
    services::IdentityService,
};
use chrono::{DateTime, Duration, Utc};
use ring::rand::SystemRandom;
use shine_infra::{
    crypto::random,
    web::{
        extracts::{ClientFingerprint, SiteInfo},
        responses::Problem,
    },
};
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum LoginTokenError {
    #[error("Retry limit reached")]
    RetryLimitReached,

    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl From<LoginTokenError> for Problem {
    fn from(err: LoginTokenError) -> Self {
        match err {
            LoginTokenError::IdentityError(err) => err.into(),

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

pub struct LoginTokenHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    random: &'a SystemRandom,
    identity_service: &'a IdentityService<IDB>,
}

impl<'a, IDB> LoginTokenHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    pub fn new(random: &'a SystemRandom, identity_service: &'a IdentityService<IDB>) -> Self {
        Self { random, identity_service }
    }

    pub async fn create_user_token(
        &self,
        user_id: Uuid,
        kind: TokenKind,
        time_to_live: &Duration,
        fingerprint_to_bind_to: Option<&ClientFingerprint>,
        site_info: &SiteInfo,
    ) -> Result<UserToken, LoginTokenError> {
        const MAX_RETRY_COUNT: usize = 10;

        let mut retry_count = 0;
        loop {
            log::debug!("Creating new {kind:?} token for user {user_id}, retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(LoginTokenError::RetryLimitReached);
            }
            retry_count += 1;

            let token = random::hex_16(self.random);
            match self
                .identity_service
                .add_token(
                    user_id,
                    kind,
                    &token,
                    time_to_live,
                    fingerprint_to_bind_to,
                    None,
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
                Err(err) => return Err(LoginTokenError::IdentityError(err)),
            }
        }
    }
}

impl AppState {
    pub fn login_token_handler(&self) -> LoginTokenHandler<impl IdentityDb> {
        LoginTokenHandler::new(self.random(), self.identity_service())
    }
}
