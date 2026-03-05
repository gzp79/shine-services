use crate::{
    app_config::{AppConfig, IdEncoderConfig, MailerConfig},
    repositories::{
        identity::pg::PgIdentityDb,
        mailer::{smtp::SmtpEmailSender, EmailSender},
        session::{redis::RedisSessionDb, SessionDb},
        CaptchaValidator, DBPool,
    },
    services::{
        IdentityTopic, LinkService, MailerService, RoleService, SessionService, SettingsService, TokenService,
        TokenSettings, UserService,
    },
};
use anyhow::{anyhow, Error as AnyError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use chrono::Duration;
use ring::{aead, rand::SystemRandom};
use shine_infra::{
    crypto::{HarshIdEncoder, IdEncoder, OptimusIdEncoder, PrefixedIdEncoder},
    sync::TopicBus,
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
    session_service: SessionService<RedisSessionDb>,
    email_sender: SmtpEmailSender,
    // Phase 2 services
    events: Arc<TopicBus<IdentityTopic>>,
    user_service: UserService<PgIdentityDb>,
    token_service: TokenService<PgIdentityDb>,
    role_service: RoleService<PgIdentityDb>,
    link_service: LinkService<PgIdentityDb>,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(config: &WebAppConfig<AppConfig>) -> Result<Self, AnyError> {
        let config_auth = &config.feature.auth;
        let config_db = &config.feature.db;
        let config_user_name = &config.feature.name;

        let settings = {
            let allowed_redirect_urls = config_auth
                .allowed_redirect_urls
                .iter()
                .map(|r| regex::Regex::new(r).map_err(|e| anyhow!(e)))
                .collect::<Result<Vec<_>, _>>()?;
            if allowed_redirect_urls.is_empty() {
                return Err(anyhow!("allowed_redirect_urls is empty"));
            }

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
                allowed_redirect_urls,
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

        // Phase 2 services
        let events = Arc::new(TopicBus::<IdentityTopic>::new());

        let user_service = {
            let identity_db = PgIdentityDb::new(&db_pool.postgres, &config_db.email_protection).await?;
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
            UserService::new(identity_db, user_name_generator, Arc::clone(&events))
        };

        let token_service = {
            let identity_db = PgIdentityDb::new(&db_pool.postgres, &config_db.email_protection).await?;
            TokenService::new(identity_db)
        };

        let role_service = {
            let identity_db = PgIdentityDb::new(&db_pool.postgres, &config_db.email_protection).await?;
            RoleService::new(identity_db, Arc::clone(&events))
        };

        let link_service = {
            let identity_db = PgIdentityDb::new(&db_pool.postgres, &config_db.email_protection).await?;
            LinkService::new(identity_db, Arc::clone(&events))
        };

        Ok(Self(Arc::new(Inner {
            settings,
            problem_config,
            random: SystemRandom::new(),
            tera,
            db: db_pool,
            captcha_validator,
            session_service,
            email_sender,
            events,
            user_service,
            token_service,
            role_service,
            link_service,
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

    pub fn session_service(&self) -> &SessionService<impl SessionDb> {
        &self.0.session_service
    }

    pub fn random(&self) -> &SystemRandom {
        &self.0.random
    }

    pub fn mailer_service(&self) -> MailerService<'_, impl EmailSender> {
        MailerService::new(&self.0.settings, &self.0.email_sender, &self.0.tera)
    }

    // Phase 2 service getters
    pub fn events(&self) -> &Arc<TopicBus<IdentityTopic>> {
        &self.0.events
    }

    pub fn user_service(&self) -> &UserService<PgIdentityDb> {
        &self.0.user_service
    }

    pub fn token_service(&self) -> &TokenService<PgIdentityDb> {
        &self.0.token_service
    }

    pub fn role_service(&self) -> &RoleService<PgIdentityDb> {
        &self.0.role_service
    }

    pub fn link_service(&self) -> &LinkService<PgIdentityDb> {
        &self.0.link_service
    }

    // User info aggregation helpers (moved from UserInfoHandler)

    pub async fn get_user_info(&self, user_id: Uuid) -> Result<Option<UserInfo>, UserInfoError> {
        // get the version first as newer role is fine, but a deprecated role set is not ok
        // this order ensures the role and other data are at least as fresh as the version

        let identity = match self.user_service().find_by_id(user_id).await? {
            Some(identity) => identity,
            None => return Ok(None),
        };

        let is_linked = self.link_service().is_linked(user_id).await?;

        let roles = match self.role_service().get_roles(user_id).await? {
            Some(roles) => roles,
            None => return Ok(None),
        };

        Ok(Some(UserInfo { identity, roles, is_linked }))
    }

    pub async fn create_user_session(
        &self,
        identity: &Identity,
        fingerprint: &ClientFingerprint,
        site_info: &SiteInfo,
    ) -> Result<Option<CurrentUser>, UserInfoError> {
        let is_linked = self.link_service().is_linked(identity.id).await?;
        let roles = match self.role_service().get_roles(identity.id).await? {
            Some(roles) => roles,
            None => return Ok(None),
        };

        // Create session
        log::debug!("Creating session for identity: {identity:#?}");
        let (user_session, user_session_key) = self
            .session_service()
            .create(identity, roles, is_linked, fingerprint, site_info)
            .await?;

        Ok(Some(CurrentUser {
            user_id: user_session.info.user_id,
            key: user_session_key,
            session_start: user_session.info.created_at,
            session_end: user_session.expire_at,
            name: user_session.user.name,
            roles: user_session.user.roles,
            is_email_confirmed: user_session.user.is_email_confirmed,
            is_linked: user_session.user.is_linked,
            fingerprint: user_session.info.fingerprint,
        }))
    }

    pub async fn refresh_user_session(&self, user_id: Uuid) -> Result<(), UserInfoError> {
        match self.get_user_info(user_id).await {
            Ok(Some(user_info)) => {
                // at this point the identity DB has been updated, thus any new session will contain the information
                // not older than the user info just request, thus it should be not an issue if a user signs in
                // during this update process. If there is a frequent update the version should trigger an
                // refresh on the session anyway.
                self.session_service()
                    .update_all(&user_info.identity, &user_info.roles, user_info.is_linked)
                    .await?;
                Ok(())
            }
            Ok(None) => {
                log::warn!("User ({user_id}) not found, removing all the sessions");
                self.session_service().remove_all(user_id).await?;
                Ok(())
            }
            Err(err) => {
                log::warn!("Failed to refresh session for user ({err}):");
                //self.session_service().remove_all(user_id).await?; - keep sessions, it could be a temporary issue
                Err(err)
            }
        }
    }
}

// Types moved from UserInfoHandler
use crate::repositories::identity::{Identity, IdentityError};
use crate::repositories::session::SessionError;
use shine_infra::web::{
    extracts::{ClientFingerprint, SiteInfo},
    session::CurrentUser,
};
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(ThisError, Debug)]
pub enum UserInfoError {
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
    #[error(transparent)]
    SessionError(#[from] SessionError),
}

impl From<UserInfoError> for shine_infra::web::responses::Problem {
    fn from(value: UserInfoError) -> Self {
        match value {
            UserInfoError::IdentityError(err) => err.into(),
            UserInfoError::SessionError(err) => err.into(),
        }
    }
}

pub struct UserInfo {
    pub identity: Identity,
    pub roles: Vec<String>,
    pub is_linked: bool,
}
