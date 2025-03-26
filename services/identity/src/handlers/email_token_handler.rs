use crate::{
    app_state::AppState,
    repositories::{
        identity::{IdentityDb, IdentityError},
        mailer::{EmailSender, EmailSenderError},
    },
    services::{IdentityService, MailerService, SettingsService},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use chrono::{DateTime, Utc};
use ring::{
    aead::{self, Nonce},
    rand::{SecureRandom, SystemRandom},
};
use shine_infra::{language::Language, web::Problem};
use thiserror::Error as ThisError;
use uuid::Uuid;

const TOKEN_EXPIRED: &str = "email-token-expired";
const INVALID_TOKEN: &str = "email-invalid-token";
const MISSING_EMAIL: &str = "email-missing-email";
const EMAIL_CONFLICT: &str = "email-conflict";

#[derive(Debug, ThisError)]
pub enum EmailTokenError {
    #[error("Invalid token")]
    InvalidToken,
    #[error("Token expired")]
    TokenExpired,
    #[error("User in token is not matching")]
    TokenWrongUser,
    #[error("Email in token is not matching")]
    TokenWrongEmail,
    #[error("No email to validate")]
    MissingEmail,
    #[error("Email already in use")]
    EmailConflict,
    #[error("Encryption error")]
    EncryptionError,
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
    #[error(transparent)]
    EmailSenderError(#[from] EmailSenderError),
}

impl From<EmailTokenError> for Problem {
    fn from(err: EmailTokenError) -> Self {
        match err {
            EmailTokenError::InvalidToken => Problem::bad_request(INVALID_TOKEN),
            EmailTokenError::TokenExpired => Problem::bad_request(TOKEN_EXPIRED).with_sensitive("tokenExpired"),
            EmailTokenError::TokenWrongUser => Problem::bad_request(TOKEN_EXPIRED).with_sensitive("wrongUser"),
            EmailTokenError::TokenWrongEmail => Problem::bad_request(TOKEN_EXPIRED).with_sensitive("wrongEmail"),
            EmailTokenError::MissingEmail => Problem::precondition_failed(MISSING_EMAIL),
            EmailTokenError::EmailConflict => Problem::precondition_failed(EMAIL_CONFLICT),
            EmailTokenError::IdentityError(IdentityError::UserDeleted { .. }) => {
                Problem::unauthorized_ty(TOKEN_EXPIRED).with_sensitive("userDeleted")
            }
            EmailTokenError::EncryptionError => Problem::internal_error().with_sensitive("encryptionError"),
            EmailTokenError::IdentityError(error) => Problem::internal_error().with_sensitive(Problem::from(error)),
            EmailTokenError::EmailSenderError(error) => Problem::internal_error().with_sensitive(Problem::from(error)),
        }
    }
}

pub struct EmailTokenHandler<'a, IDB, EMS>
where
    IDB: IdentityDb,
    EMS: EmailSender,
{
    random: &'a SystemRandom,
    settings: &'a SettingsService,
    identity_service: &'a IdentityService<IDB>,
    mailer_service: MailerService<'a, EMS>,
}

impl<'a, IDB, EMS> EmailTokenHandler<'a, IDB, EMS>
where
    IDB: IdentityDb,
    EMS: EmailSender,
{
    fn encrypt(
        &self,
        user_id: Uuid,
        current_email: &str,
        new_email: &str,
        expire_at: &DateTime<Utc>,
    ) -> Result<String, EmailTokenError> {
        let expire_at = expire_at.timestamp();

        let mut raw_nonce = [0u8; 12];
        raw_nonce[0..8].copy_from_slice(&expire_at.to_le_bytes());
        self.random
            .fill(&mut raw_nonce[8..])
            .map_err(|_| EmailTokenError::EncryptionError)?;

        let mut in_out = format!("{},{},{}", user_id, current_email, new_email).into_bytes();
        // in theory neither part of the message should contain a ",", thus there should be exactly 2 of them
        // just a sanity check to avoid any potential decryption issues
        if in_out.iter().filter(|&&c| c == b',').count() != 2 {
            return Err(EmailTokenError::EncryptionError);
        }

        let key = &self.settings.token.email_key;
        let nonce = Nonce::assume_unique_for_key(raw_nonce);
        key.seal_in_place_append_tag(nonce, aead::Aad::from(&[]), &mut in_out)
            .map_err(|_| EmailTokenError::EncryptionError)?;
        in_out.extend_from_slice(&raw_nonce);

        Ok(B64.encode(&in_out))
    }

    fn decrypt(&self, token: &[u8]) -> Result<(Uuid, String, String), EmailTokenError> {
        let cipher_text = B64.decode(token).map_err(|_| EmailTokenError::InvalidToken)?;

        let nonce_offset = cipher_text.len().checked_sub(12).ok_or(EmailTokenError::InvalidToken)?;
        let (cipher_text, nonce) = cipher_text.split_at(nonce_offset);

        let tag_offset = cipher_text.len().checked_sub(16).ok_or(EmailTokenError::InvalidToken)?;
        let (cipher_text, tag) = cipher_text.split_at(tag_offset);

        let expire_at = i64::from_le_bytes(nonce[0..8].try_into().map_err(|_| EmailTokenError::InvalidToken)?);
        let expire_at = DateTime::<Utc>::from_timestamp(expire_at, 0).ok_or(EmailTokenError::InvalidToken)?;
        if expire_at < Utc::now() {
            return Err(EmailTokenError::TokenExpired);
        }

        let mut in_out: Vec<u8> = cipher_text.to_vec();

        let key = &self.settings.token.email_key;
        let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| EmailTokenError::InvalidToken)?;
        let tag = aead::Tag::try_from(tag).map_err(|_| EmailTokenError::InvalidToken)?;
        key.open_in_place_separate_tag(nonce, aead::Aad::from(&[]), tag, &mut in_out, 0..)
            .map_err(|_| EmailTokenError::InvalidToken)?;
        let data = String::from_utf8(in_out).map_err(|_| EmailTokenError::InvalidToken)?;

        let mut tokens = data.split(',');
        let user_id = Uuid::parse_str(tokens.next().ok_or(EmailTokenError::InvalidToken)?)
            .map_err(|_| EmailTokenError::InvalidToken)?;
        let current_email = tokens.next().ok_or(EmailTokenError::InvalidToken)?.to_string();
        let new_email = tokens.next().ok_or(EmailTokenError::InvalidToken)?.to_string();
        Ok((user_id, current_email, new_email))
    }

    pub async fn start_email_confirm_flow(&self, user_id: Uuid, lang: Option<Language>) -> Result<(), EmailTokenError> {
        let user = self
            .identity_service
            .find_by_id(user_id)
            .await?
            .ok_or(IdentityError::UserDeleted { id: user_id })?;
        let email = user.email.as_ref().ok_or(EmailTokenError::MissingEmail)?;

        let ttl = self.settings.token.ttl_email_token;
        let expire_at = Utc::now() + ttl;
        let token = self.encrypt(user_id, email, email, &expire_at)?;

        self.mailer_service
            .send_email_confirmation(email, &token, lang, &user.name)
            .await?;

        Ok(())
    }

    pub async fn start_email_change_flow(
        &self,
        user_id: Uuid,
        new_email: &str,
        lang: Option<Language>,
    ) -> Result<(), EmailTokenError> {
        let user = self
            .identity_service
            .find_by_id(user_id)
            .await?
            .ok_or(IdentityError::UserDeleted { id: user_id })?;
        let old_email = user.email.as_deref().unwrap_or("");

        let ttl = self.settings.token.ttl_email_token;
        let expire_at = Utc::now() + ttl;
        let token = self.encrypt(user_id, old_email, new_email, &expire_at)?;

        self.mailer_service
            .send_email_change(new_email, &token, lang, &user.name)
            .await?;

        Ok(())
    }

    pub async fn complete_email_flow(&self, user_id: Uuid, token: &str) -> Result<(), EmailTokenError> {
        let (token_user_id, token_old_email, token_new_email) = self.decrypt(token.as_bytes())?;
        if user_id != token_user_id {
            return Err(EmailTokenError::TokenWrongUser);
        }

        let user = self
            .identity_service
            .find_by_id(user_id)
            .await?
            .ok_or(IdentityError::UserDeleted { id: user_id })?;

        let old_email = user.email.as_deref().unwrap_or("");
        if old_email != token_old_email {
            return Err(EmailTokenError::TokenWrongEmail);
        }

        match self
            .identity_service
            .update(user_id, None, Some((&token_new_email, true)))
            .await
        {
            Ok(_) => (),
            Err(IdentityError::EmailConflict) => return Err(EmailTokenError::EmailConflict),
            Err(err) => return Err(EmailTokenError::IdentityError(err)),
        }

        Ok(())
    }
}

impl AppState {
    pub fn email_token_handler(&self) -> EmailTokenHandler<impl IdentityDb, impl EmailSender> {
        EmailTokenHandler {
            random: self.random(),
            settings: self.settings(),
            identity_service: self.identity_service(),
            mailer_service: self.mailer_service(),
        }
    }
}
