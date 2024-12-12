use crate::{
    app_config::{AppConfig, IdEncoderConfig},
    repositories::{
        identity::{pg::PgIdentityDb, IdentityDb},
        session::{redis::RedisSessionDb, SessionDb},
        CaptchaValidator, DBPool,
    },
    services::{
        CreateUserService, IdentityService, Permission, PermissionError, PermissionSet, SessionService,
        SessionUserSyncService, TokenGenerator,
    },
};
use anyhow::{anyhow, Error as AnyError};
use chrono::Duration;
use ring::rand::SystemRandom;
use shine_service::{
    axum::{telemetry::TelemetryService, IntoProblem, Problem, ProblemConfig},
    service::CurrentUser,
    utils::{HarshIdEncoder, IdEncoder, OptimusIdEncoder, PrefixedIdEncoder},
};
use std::sync::Arc;
use tera::Tera;
use url::Url;

pub struct TokenSettings {
    pub ttl_access_token: Duration,
    pub ttl_single_access: Duration,
    pub ttl_api_key: Duration,
}

pub struct AppSettings {
    pub app_name: String,
    pub home_url: Url,
    pub error_url: Url,
    pub auth_base_url: Url,
    pub token: TokenSettings,
    pub external_providers: Vec<String>,
    pub full_problem_response: bool,
    pub page_redirect_time: Option<u32>,
    pub super_user_api_key_hash: Option<String>,
}

struct Inner {
    settings: AppSettings,
    problem_config: ProblemConfig,
    random: SystemRandom,
    tera: Tera,
    db: DBPool,
    captcha_validator: CaptchaValidator,
    telemetry_service: TelemetryService,
    identity_service: IdentityService<PgIdentityDb>,
    session_service: SessionService<RedisSessionDb>,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(config: &AppConfig, telemetry_service: &TelemetryService) -> Result<Self, AnyError> {
        let settings = AppSettings {
            app_name: config.auth.app_name.clone(),
            home_url: config.auth.home_url.clone(),
            error_url: config.auth.error_url.clone(),
            auth_base_url: config.auth.auth_base_url.clone(),
            token: TokenSettings {
                ttl_access_token: Duration::seconds(i64::try_from(config.auth.auth_session.ttl_access_token)?),
                ttl_single_access: Duration::seconds(i64::try_from(config.auth.auth_session.ttl_single_access)?),
                ttl_api_key: Duration::seconds(i64::try_from(config.auth.auth_session.ttl_api_key)?),
            },
            external_providers: todo!(),
            full_problem_response: config.service.full_problem_response,
            page_redirect_time: config.auth.page_redirect_time,
            super_user_api_key_hash: config.auth.super_user_api_key_hash.clone(),
        };
        let problem_config = ProblemConfig::new(config.service.full_problem_response);

        let tera = {
            let mut tera = Tera::new("tera_templates/**/*").map_err(|e| anyhow!(e))?;
            tera.autoescape_on(vec![".html"]);
            tera
        };

        let db_pool = DBPool::new(&config.db).await?;
        let captcha_validator = CaptchaValidator::new(&config.service.captcha_secret);

        let identity_service = {
            let identity_db = PgIdentityDb::new(&db_pool.postgres).await?;
            let user_name_generator: Box<dyn IdEncoder> = match &config.user_name.id_encoder {
                IdEncoderConfig::Optimus { prime, random } => Box::new(PrefixedIdEncoder::new(
                    &config.user_name.base_name,
                    OptimusIdEncoder::new(*prime, *random),
                )),
                IdEncoderConfig::Harsh { salt } => Box::new(PrefixedIdEncoder::new(
                    &config.user_name.base_name,
                    HarshIdEncoder::new(salt)?,
                )),
            };
            IdentityService::new(identity_db, user_name_generator)
        };

        let session_service = {
            let ttl_session = Duration::seconds(i64::try_from(config.auth.auth_session.ttl_session)?);
            let session_db = RedisSessionDb::new(&db_pool.redis, "".to_string(), ttl_session).await?;
            SessionService::new(session_db)
        };

        Ok(Self(Arc::new(Inner {
            settings,
            problem_config,
            random: SystemRandom::new(),
            tera,
            db: db_pool,
            captcha_validator,
            telemetry_service: telemetry_service.clone(),
            identity_service,
            session_service,
        })))
    }

    pub fn settings(&self) -> &AppSettings {
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

    pub fn telemetry_service(&self) -> &TelemetryService {
        &self.0.telemetry_service
    }

    pub fn identity_service(&self) -> &IdentityService<impl IdentityDb> {
        &self.0.identity_service
    }

    pub fn session_service(&self) -> &SessionService<impl SessionDb> {
        &self.0.session_service
    }

    pub fn create_user_service(&self) -> CreateUserService<impl IdentityDb, impl SessionDb> {
        CreateUserService::new(self.identity_service(), self.session_service())
    }

    pub fn session_user_sync_service(&self) -> SessionUserSyncService<impl IdentityDb, impl SessionDb> {
        SessionUserSyncService::new(self.identity_service(), self.session_service())
    }

    pub fn token_generator_service(&self) -> TokenGenerator<impl IdentityDb> {
        TokenGenerator::new(&self.0.random, self.identity_service())
    }
}

impl AppState {
    pub async fn require_permission(
        &self,
        current_user: &CurrentUser,
        permission: Permission,
    ) -> Result<(), PermissionError> {
        // At the moment role -> permission mapping is hardcoded, but it could be stored in the database,
        // so the function was made async.
        PermissionSet::from(current_user).require(permission)?;
        Ok(())
    }

    pub async fn check_permission(&self, current_user: &CurrentUser, permission: Permission) -> Result<(), Problem> {
        self.require_permission(current_user, permission)
            .await
            .map_err(|err| err.into_problem(self.problem_config()))?;
        Ok(())
    }
}
