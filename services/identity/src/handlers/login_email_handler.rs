use crate::{
    app_state::AppState,
    repositories::{
        identity::{Identity, IdentityDb, IdentityError, TokenKind},
        mailer::{EmailSender, EmailSenderError},
    },
    services::{hash_email, CreateUserError, MailerService, SettingsService, TokenError, TokenService, UserService},
};
use shine_infra::{
    language::Language,
    web::{extracts::SiteInfo, responses::Problem},
};
use thiserror::Error as ThisError;
use url::Url;

#[derive(Debug, ThisError)]
pub enum LoginEmailError {
    #[error(transparent)]
    TokenError(#[from] TokenError),

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
            LoginEmailError::TokenError(TokenError::IdentityError(err)) => err.into(),
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
    settings_service: &'a SettingsService,
    user_service: &'a UserService<IDB>,
    token_service: &'a TokenService<IDB>,
    mailer_service: MailerService<'a, EMS>,
}

impl<'a, IDB, EMS> LoginEmailHandler<'a, IDB, EMS>
where
    IDB: IdentityDb,
    EMS: EmailSender,
{
    pub fn new(
        user_service: &'a UserService<IDB>,
        token_service: &'a TokenService<IDB>,
        settings_service: &'a SettingsService,
        mailer_service: MailerService<'a, EMS>,
    ) -> Self {
        Self {
            user_service,
            token_service,
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
                        None => return Err(LoginEmailError::IdentityError(IdentityError::UserDeleted)),
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
    pub fn login_email_handler(&self) -> LoginEmailHandler<'_, impl IdentityDb, impl EmailSender> {
        LoginEmailHandler::new(
            self.user_service(),
            self.token_service(),
            self.settings(),
            self.mailer_service(),
        )
    }
}
