use crate::services::SettingsService;
use chrono::{DateTime, Duration, Utc};
use ring::hmac;
use shine_core::web::Problem;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum EmailTokenServiceError {
    #[error("Invalid token")]
    InvalidToken,
    #[error("Token expired")]
    TokenExpired,
    #[error("Token expired")]
    TokenMissMatching,
}

impl From<EmailTokenServiceError> for Problem {
    fn from(err: EmailTokenServiceError) -> Self {
        match err {
            EmailTokenServiceError::InvalidToken => Problem::bad_request("auth-invalid-token"),
            EmailTokenServiceError::TokenExpired => {
                Problem::bad_request("auth-token-expired").with_sensitive("tokenExpired")
            }
            EmailTokenServiceError::TokenMissMatching => {
                Problem::bad_request("auth-token-expired").with_sensitive("tokenMissMatch")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct EmailToken {
    pub token: String,
}

pub struct EmailTokenService<'a> {
    settings: &'a SettingsService,
}

impl<'a> EmailTokenService<'a> {
    pub fn new(settings: &'a SettingsService) -> Self {
        Self { settings }
    }

    fn generate_email_verify_data(
        &self,
        user_id: Uuid,
        current_email: &str,
        new_email: &str,
        expire_at: &DateTime<Utc>,
    ) -> String {
        format!("{},{},{},{}", user_id, current_email, new_email, expire_at.timestamp())
    }

    pub async fn create_email_verify_token(
        &self,
        user_id: Uuid,
        time_to_live: &Duration,
        email: &str,
    ) -> Result<EmailToken, EmailTokenServiceError> {
        let expire_at = Utc::now() + *time_to_live;

        let msg = self.generate_email_verify_data(user_id, email, email, &expire_at);
        let token = hmac::sign(&self.settings.token.email_key, msg.as_bytes());
        log::trace!("Signature for [{}]: {:?}", msg, token);
        let token_hex = hex::encode(token.as_ref());
        let token = format!("{};{:x}", token_hex, expire_at.timestamp());

        Ok(EmailToken { token })
    }

    pub async fn check_email_verify_token(
        &self,
        user_id: Uuid,
        email: &str,
        token: &str,
    ) -> Result<(), EmailTokenServiceError> {
        let (signature, expire_at) = token.split_once(';').ok_or(EmailTokenServiceError::InvalidToken)?;

        let signature = hex::decode(signature).map_err(|_| EmailTokenServiceError::InvalidToken)?;

        let expire_at = i64::from_str_radix(expire_at, 16).map_err(|_| EmailTokenServiceError::InvalidToken)?;
        let expire_at = DateTime::<Utc>::from_timestamp(expire_at, 0).ok_or(EmailTokenServiceError::InvalidToken)?;
        if expire_at < Utc::now() {
            return Err(EmailTokenServiceError::TokenExpired);
        }

        let msg = self.generate_email_verify_data(user_id, email, email, &expire_at);
        log::trace!("Verify signature for [{}]", msg);
        if hmac::verify(&self.settings.token.email_key, msg.as_bytes(), &signature).is_err() {
            Err(EmailTokenServiceError::TokenMissMatching)
        } else {
            Ok(())
        }
    }
}
