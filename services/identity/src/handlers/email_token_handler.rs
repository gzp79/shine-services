use crate::{
    app_state::AppState,
    models::{Identity, IdentityError, TokenKind},
    repositories::{
        identity::IdentityDb,
        mailer::{EmailSender, EmailSenderError},
    },
    services::{CreateUserError, MailerService, SettingsService, TokenError, TokenService, UserService},
};
use shine_infra::{
    language::Language,
    models::hash_email,
    web::{extracts::SiteInfo, responses::Problem},
};
use thiserror::Error as ThisError;
use url::Url;
use uuid::Uuid;

const TOKEN_EXPIRED: &str = "email-token-expired";
const INVALID_TOKEN: &str = "email-invalid-token";
const MISSING_EMAIL: &str = "email-missing-email";
const EMAIL_CONFLICT: &str = "email-conflict";

#[derive(Debug, ThisError)]
pub enum EmailAuthError {
    #[error("Invalid token")]
    InvalidToken,
    #[error("Token expired")]
    TokenExpired,
    #[error("User in token is not matching")]
    TokenWrongUser,
    #[error("No email to validate")]
    MissingEmail,
    #[error("Email already in use")]
    EmailConflict,
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
    #[error(transparent)]
    CreateUserError(#[from] CreateUserError),
    #[error(transparent)]
    TokenError(#[from] TokenError),
    #[error(transparent)]
    EmailSenderError(#[from] EmailSenderError),
}

impl From<EmailAuthError> for Problem {
    fn from(err: EmailAuthError) -> Self {
        match err {
            EmailAuthError::InvalidToken => Problem::bad_request(INVALID_TOKEN),
            EmailAuthError::TokenExpired => Problem::bad_request(TOKEN_EXPIRED).with_sensitive("tokenExpired"),
            EmailAuthError::TokenWrongUser => Problem::bad_request(TOKEN_EXPIRED).with_sensitive("wrongUser"),
            EmailAuthError::MissingEmail => Problem::precondition_failed(MISSING_EMAIL),
            EmailAuthError::EmailConflict => Problem::precondition_failed(EMAIL_CONFLICT),
            EmailAuthError::IdentityError(IdentityError::UserDeleted) => {
                Problem::unauthorized_ty(TOKEN_EXPIRED).with_sensitive("userDeleted")
            }
            EmailAuthError::CreateUserError(CreateUserError::IdentityError(err)) => Problem::from(err),
            EmailAuthError::IdentityError(error) => Problem::internal_error().with_sensitive(Problem::from(error)),
            EmailAuthError::CreateUserError(error) => Problem::internal_error()
                .with_detail(error.to_string())
                .with_sensitive_dbg(error),
            EmailAuthError::TokenError(TokenError::IdentityError(err)) => Problem::from(err),
            EmailAuthError::TokenError(error) => Problem::internal_error().with_sensitive(format!("{error}")),
            EmailAuthError::EmailSenderError(error) => Problem::internal_error().with_sensitive(Problem::from(error)),
        }
    }
}

pub struct EmailAuthHandler<'a, IDB, EMS>
where
    IDB: IdentityDb,
    EMS: EmailSender,
{
    settings: &'a SettingsService,
    user_service: &'a UserService<IDB>,
    token_service: &'a TokenService<IDB>,
    mailer_service: MailerService<'a, EMS>,
}

impl<'a, IDB, EMS> EmailAuthHandler<'a, IDB, EMS>
where
    IDB: IdentityDb,
    EMS: EmailSender,
{
    pub fn new(
        settings: &'a SettingsService,
        user_service: &'a UserService<IDB>,
        token_service: &'a TokenService<IDB>,
        mailer_service: MailerService<'a, EMS>,
    ) -> Self {
        Self {
            settings,
            user_service,
            token_service,
            mailer_service,
        }
    }

    /// Send login/registration email
    /// Creates user if doesn't exist, then sends email with login token
    pub async fn send_login_email(
        &self,
        email: &str,
        remember_me: Option<bool>,
        redirect_url: Option<&Url>,
        site_info: &SiteInfo,
        lang: Option<Language>,
    ) -> Result<Identity, EmailAuthError> {
        let (is_registration, identity) = {
            match self.user_service.create_with_retry(None, Some(email)).await {
                Ok(identity) => {
                    log::debug!("New user created through email flow: {identity:#?}");
                    (true, identity)
                }
                Err(CreateUserError::IdentityError(IdentityError::EmailConflict)) => {
                    match self.user_service.find_by_email(email).await? {
                        Some(identity) => {
                            log::debug!("User found by email: {identity:#?}");
                            (false, identity)
                        }
                        None => return Err(EmailAuthError::IdentityError(IdentityError::UserDeleted)),
                    }
                }
                Err(err) => return Err(err.into()),
            }
        };

        let time_to_live = if is_registration {
            // New user - allow longer time to register
            self.settings.token.ttl_access_token
        } else {
            // Existing user - standard login token TTL
            self.settings.token.ttl_email_login_token
        };

        let (token, _token_info) = self
            .token_service
            .create_with_retry(
                identity.id,
                TokenKind::EmailAccess,
                &time_to_live,
                None,
                Some(email),
                site_info,
            )
            .await?;

        let mut link_url = self
            .settings
            .auth_base_url
            .join("token/login")
            .map_err(|err| EmailSenderError::SendFailed(err.to_string()))?;

        {
            let mut query = link_url.query_pairs_mut();
            query.clear();
            query.append_pair("token", &token);
            query.append_pair("captcha", &hash_email(email));

            if let Some(remember_me) = remember_me {
                query.append_pair("rememberMe", &remember_me.to_string());
            }
            if let Some(redirect_url) = redirect_url {
                query.append_pair("redirectUrl", redirect_url.as_str());
            }
        }

        if is_registration {
            self.mailer_service
                .send_email_register(email, link_url, &identity.name, lang)
                .await?;
        } else {
            self.mailer_service
                .send_email_login(email, link_url, &identity.name, lang)
                .await?;
        }

        Ok(identity)
    }

    /// Send email confirmation link
    pub async fn start_email_confirm_flow(
        &self,
        user_id: Uuid,
        site_info: &SiteInfo,
        lang: Option<Language>,
    ) -> Result<(), EmailAuthError> {
        let user = self
            .user_service
            .find_by_id(user_id)
            .await?
            .ok_or(IdentityError::UserDeleted)?;
        let email = user.email.as_ref().ok_or(EmailAuthError::MissingEmail)?;

        let ttl = self.settings.token.ttl_email_login_token;

        // Create token via TokenService - automatically deletes old email tokens (uniqueness rule)
        let (token, _token_info) = self
            .token_service
            .create_with_retry(
                user_id,
                TokenKind::EmailAccess,
                &ttl,
                None,        // No fingerprint binding
                Some(email), // Bind to email being confirmed
                site_info,
            )
            .await?;

        self.mailer_service
            .send_email_confirmation(email, &token, &user.name, lang)
            .await?;

        Ok(())
    }

    /// Send email change confirmation link
    pub async fn start_email_change_flow(
        &self,
        user_id: Uuid,
        new_email: &str,
        site_info: &SiteInfo,
        lang: Option<Language>,
    ) -> Result<(), EmailAuthError> {
        let user = self
            .user_service
            .find_by_id(user_id)
            .await?
            .ok_or(IdentityError::UserDeleted)?;

        let ttl = self.settings.token.ttl_email_login_token;

        // Create token via TokenService - automatically deletes old email tokens (uniqueness rule)
        let (token, _token_info) = self
            .token_service
            .create_with_retry(
                user_id,
                TokenKind::EmailAccess,
                &ttl,
                None,            // No fingerprint binding
                Some(new_email), // Bind to new email
                site_info,
            )
            .await?;

        self.mailer_service
            .send_email_change(new_email, &token, lang, &user.name)
            .await?;

        Ok(())
    }

    /// Complete email confirmation or change by validating token
    pub async fn complete_email_flow(&self, user_id: Uuid, token: &str) -> Result<(), EmailAuthError> {
        // Take any token type (burns wrong types for defense-in-depth)
        let token_info = self.token_service.take(TokenKind::all(), token).await?;

        let (identity, token_data) = token_info.ok_or(EmailAuthError::TokenExpired)?;

        // Verify correct token kind (wrong kinds already burned by take)
        if token_data.kind != TokenKind::EmailAccess {
            log::warn!(
                "Wrong token type used in email flow - token burned: user_id={user_id}, token_kind={:?}",
                token_data.kind
            );
            return Err(EmailAuthError::InvalidToken);
        }

        // Check if token is expired
        if token_data.is_expired {
            return Err(EmailAuthError::TokenExpired);
        }

        // Verify token belongs to this user
        if identity.id != user_id {
            return Err(EmailAuthError::TokenWrongUser);
        }

        // Get target email from token's bound email
        let new_email = token_data.bound_email.as_deref().ok_or(EmailAuthError::MissingEmail)?;

        // Update user's email and mark as confirmed
        match self.user_service.update(user_id, None, Some((new_email, true))).await {
            Ok(_) => (),
            Err(IdentityError::EmailConflict) => return Err(EmailAuthError::EmailConflict),
            Err(err) => return Err(EmailAuthError::IdentityError(err)),
        }

        Ok(())
    }
}

impl AppState {
    pub fn email_auth_handler(&self) -> EmailAuthHandler<'_, impl IdentityDb, impl EmailSender> {
        EmailAuthHandler::new(
            self.settings(),
            self.user_service(),
            self.token_service(),
            self.mailer_service(),
        )
    }
}
