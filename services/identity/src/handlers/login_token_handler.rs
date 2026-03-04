use crate::{
    app_state::AppState,
    repositories::identity::{IdentityDb, IdentityError, TokenKind},
    services::{TokenError, TokenService},
};
use chrono::{DateTime, Duration, Utc};
use shine_infra::web::{
    extracts::{ClientFingerprint, SiteInfo},
    responses::Problem,
};
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum LoginTokenError {
    #[error(transparent)]
    TokenError(#[from] TokenError),

    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl From<LoginTokenError> for Problem {
    fn from(err: LoginTokenError) -> Self {
        match err {
            LoginTokenError::TokenError(TokenError::IdentityError(err)) => err.into(),
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
    token_service: &'a TokenService<IDB>,
}

impl<'a, IDB> LoginTokenHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    pub fn new(token_service: &'a TokenService<IDB>) -> Self {
        Self { token_service }
    }

    pub async fn create_user_token(
        &self,
        user_id: Uuid,
        kind: TokenKind,
        time_to_live: &Duration,
        fingerprint_to_bind_to: Option<&ClientFingerprint>,
        site_info: &SiteInfo,
    ) -> Result<UserToken, LoginTokenError> {
        let (token, info) = self
            .token_service
            .create_with_retry(user_id, kind, time_to_live, fingerprint_to_bind_to, None, site_info)
            .await?;

        Ok(UserToken {
            user_id,
            token,
            token_hash: info.token_hash,
            expire_at: info.expire_at,
        })
    }
}

impl AppState {
    pub fn login_token_handler(&self) -> LoginTokenHandler<impl IdentityDb> {
        LoginTokenHandler::new(self.token_service())
    }
}
