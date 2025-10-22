use crate::{
    app_config::{AppConfig, IdEncoderConfig, MailerConfig},
    repositories::{
        identity::{pg::PgIdentityDb, IdentityDb},
        mailer::{smtp::SmtpEmailSender, EmailSender},
        session::{redis::RedisSessionDb, SessionDb},
        CaptchaValidator, DBPool,
    },
    services::{IdentityService, MailerService, SessionService, SettingsService, TokenSettings},
};
use anyhow::{anyhow, Error as AnyError};
use base64::{prelude::BASE64_URL_SAFE_NO_PAD as B64, Engine};
use chrono::Duration;
use ring::{aead, rand::SystemRandom};
use shine_infra::{
    crypto::{HarshIdEncoder, IdEncoder, OptimusIdEncoder, PrefixedIdEncoder},
    web::{responses::ProblemConfig, WebAppConfig},
};
use std::sync::Arc;
use tera::Tera;

struct Inner {
    settings: SettingsService,
    problem_config: ProblemConfig,
    random: SystemRandom,
    tera: Tera,
    db: DBPool,
    captcha_validator: CaptchaValidator,
    identity_service: IdentityService<PgIdentityDb>,
    session_service: SessionService<RedisSessionDb>,
    email_sender: SmtpEmailSender,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(config: &WebAppConfig<AppConfig>) -> Result<Self, AnyError> {
        let config_auth = &config.feature.auth;
        let config_db = &config.feature.db;
        let config_user_name = &config.feature.name;

        let settings = {
            let email_key = &B64
                .decode(config_auth.auth_session.email_token_secret.as_bytes())
                .map_err(|e| anyhow!(e))?;
            let email_key = aead::UnboundKey::new(&aead::AES_256_GCM, email_key).map_err(|e| anyhow!(e))?;

            SettingsService {
                app_name: config_auth.app_name.clone(),
                home_url: config_auth.home_url.clone(),
                auth_base_url: config_auth.auth_base_url.clone(),
                link_url: config_auth.link_url.clone(),
                error_url: config_auth.error_url.clone(),
                token: TokenSettings {
                    ttl_access_token: Duration::seconds(i64::try_from(config_auth.auth_session.ttl_access_token)?),
                    ttl_single_access: Duration::seconds(i64::try_from(config_auth.auth_session.ttl_single_access)?),
                    ttl_api_key: Duration::seconds(i64::try_from(config_auth.auth_session.ttl_api_key)?),
                    ttl_email_login_token: Duration::seconds(i64::try_from(config_auth.auth_session.ttl_email_token)?),
                    email_key: aead::LessSafeKey::new(email_key),
                },
                external_providers: config_auth.collect_providers(),
                page_redirect_time: config_auth.page_redirect_time,
                super_user_api_key_hash: config_auth.super_user_api_key_hash.clone(),
            }
        };

        let problem_config = ProblemConfig::new(config.service.full_problem_response);

        let tera = {
            let mut tera = Tera::new("tera_templates/**/*").map_err(|e| anyhow!(e))?;
            tera.autoescape_on(vec![".html"]);
            tera
        };

        let db_pool = DBPool::new(config_db).await?;
        let captcha_validator = CaptchaValidator::new(&config.service.captcha_secret);

        let identity_service = {
            let identity_db = PgIdentityDb::new(&db_pool.postgres, config_db).await?;
            let user_name_generator: Box<dyn IdEncoder> = match &config_user_name.id_encoder {
                IdEncoderConfig::Optimus { prime, random } => Box::new(PrefixedIdEncoder::new(
                    &config_user_name.base_name,
                    OptimusIdEncoder::new(*prime, *random),
                )),
                IdEncoderConfig::Harsh { salt } => Box::new(PrefixedIdEncoder::new(
                    &config_user_name.base_name,
                    HarshIdEncoder::new(salt)?,
                )),
            };
            IdentityService::new(identity_db, user_name_generator)
        };

        let session_service = {
            let ttl_session = Duration::seconds(i64::try_from(config.service.session_ttl)?);
            let session_db = RedisSessionDb::new(&db_pool.redis, "".to_string(), ttl_session).await?;
            SessionService::new(session_db)
        };

        let email_sender = {
            match &config.feature.mailer {
                MailerConfig::Smtp {
                    email_domain,
                    smtp_url,
                    use_tls,
                    smtp_username,
                    smtp_password,
                } => SmtpEmailSender::new(
                    email_domain,
                    smtp_url,
                    use_tls.unwrap_or(true),
                    smtp_username,
                    smtp_password,
                )
                .map_err(|e| anyhow!(e))?,
            }
        };

        Ok(Self(Arc::new(Inner {
            settings,
            problem_config,
            random: SystemRandom::new(),
            tera,
            db: db_pool,
            captcha_validator,
            identity_service,
            session_service,
            email_sender,
        })))
    }

    pub fn settings(&self) -> &SettingsService {
        &self.0.settings
    }

    pub fn problem_config(&self) -> &ProblemConfig {
        &self.0.problem_config
    }

    pub fn tera(&self) -> &Tera {
        &self.0.tera
    }

    pub fn db(&self) -> &DBPool {
        &self.0.db
    }

    pub fn captcha_validator(&self) -> &CaptchaValidator {
        &self.0.captcha_validator
    }

    pub fn identity_service(&self) -> &IdentityService<impl IdentityDb> {
        &self.0.identity_service
    }

    pub fn session_service(&self) -> &SessionService<impl SessionDb> {
        &self.0.session_service
    }

    pub fn random(&self) -> &SystemRandom {
        &self.0.random
    }

    pub fn mailer_service(&self) -> MailerService<impl EmailSender> {
        MailerService::new(&self.0.settings, &self.0.email_sender, &self.0.tera)
    }
}
