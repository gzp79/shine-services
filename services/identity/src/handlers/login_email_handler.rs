use crate::{
    app_state::AppState,
    handlers::{CreateUserError, CreateUserHandler},
    repositories::{
        identity::{Identity, IdentityDb, IdentityError, TokenKind},
        mailer::{EmailSender, EmailSenderError},
    },
    services::{hash_email, IdentityService, MailerService, SettingsService},
};
use ring::rand::SystemRandom;
use shine_infra::{
    crypto::random,
    language::Language,
    web::{extracts::SiteInfo, responses::Problem},
};
use thiserror::Error as ThisError;
use url::Url;

#[derive(Debug, ThisError)]
pub enum LoginEmailError {
    #[error("Retry limit reached")]
    RetryLimitReached,

    #[error(transparent)]
    IdentityError(#[from] IdentityError),
    #[error(transparent)]
    CreateUserError(#[from] CreateUserError),
    #[error(transparent)]
    EmailSenderError(#[from] EmailSenderError),
}

impl From<LoginEmailError> for Problem {
    fn from(err: LoginEmailError) -> Self {
        match err {
            LoginEmailError::IdentityError(err) => err.into(),

            err => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}

pub struct LoginEmailHandler<'a, IDB, EMS>
where
    IDB: IdentityDb,
    EMS: EmailSender,
{
    random: &'a SystemRandom,
    settings_service: &'a SettingsService,
    identity_service: &'a IdentityService<IDB>,
    mailer_service: MailerService<'a, EMS>,
}

impl<'a, IDB, EMS> LoginEmailHandler<'a, IDB, EMS>
where
    IDB: IdentityDb,
    EMS: EmailSender,
{
    pub fn new(
        random: &'a SystemRandom,
        identity_service: &'a IdentityService<IDB>,
        settings_service: &'a SettingsService,
        mailer_service: MailerService<'a, EMS>,
    ) -> Self {
        Self {
            random,
            identity_service,
            settings_service,
            mailer_service,
        }
    }

    pub async fn send_login_email(
        &self,
        email: &str,
        remember_me: Option<bool>,
        redirect_url: Option<&Url>,
        site_info: &SiteInfo,
        lang: Option<Language>,
    ) -> Result<Identity, LoginEmailError> {
        const MAX_RETRY_COUNT: usize = 10;

        let (is_registration, identity) = {
            match CreateUserHandler::new(self.identity_service)
                .create_user(None, None, Some(email))
                .await
            {
                Ok(identity) => {
                    log::debug!("New user created through email flow: {identity:#?}");
                    (true, identity)
                }
                Err(CreateUserError::IdentityError(IdentityError::EmailConflict)) => {
                    match self.identity_service.find_by_email(email).await? {
                        Some(identity) => {
                            log::debug!("User found by email: {identity:#?}");
                            (false, identity)
                        }
                        None => {
                            return Err(LoginEmailError::IdentityError(IdentityError::UserDeleted))
                        }
                    }
                }
                Err(err) => return Err(err.into()),
            }
        };

        let time_to_live = if is_registration {
            self.settings_service.token.ttl_email_login_token
        } else {
            // this is a new user, allow him/her to register for a longer time
            self.settings_service.token.ttl_access_token
        };

        let mut retry_count = 0;
        let token = loop {
            log::debug!(
                "Creating new EmailAccess token for user {}, retry: {retry_count:#?}",
                identity.id
            );
            if retry_count > MAX_RETRY_COUNT {
                return Err(LoginEmailError::RetryLimitReached);
            }
            retry_count += 1;

            let token = random::hex_16(self.random);
            match self
                .identity_service
                .add_token(
                    identity.id,
                    TokenKind::EmailAccess,
                    &token,
                    &time_to_live,
                    None,
                    Some(email),
                    site_info,
                )
                .await
            {
                Ok(_) => break token,
                Err(IdentityError::TokenConflict) => continue,
                Err(err) => return Err(LoginEmailError::IdentityError(err)),
            }
        };

        let mut link_url = self
            .settings_service
            .auth_base_url
            .join("token/login")
            .map_err(|err| EmailSenderError::SendFailed(err.to_string()))?;

        {
            let mut query = link_url.query_pairs_mut();
            query.clear();
            query.append_pair("token", &token);

            let email_hash = hash_email(email);

            query.append_pair("captcha", &email_hash);

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
}

impl AppState {
    pub fn login_email_handler(&self) -> LoginEmailHandler<impl IdentityDb, impl EmailSender> {
        LoginEmailHandler::new(
            self.random(),
            self.identity_service(),
            self.settings(),
            self.mailer_service(),
        )
    }
}
