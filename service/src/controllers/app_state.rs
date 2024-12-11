use crate::{
    app_config::{AppConfig, IdEncoderConfig},
    repositories::{
        identity::{pg::PgIdentityDb, IdentityDb},
        session::{redis::RedisSessionDb, SessionDb},
        DBPool,
    },
    services::{IdentityService, Permission, PermissionError, PermissionSet, SessionService, SessionUserSyncService},
};
use anyhow::{anyhow, Error as AnyError};
use chrono::Duration;
use shine_service::{
    axum::{telemetry::TelemetryService, IntoProblem, Problem, ProblemConfig},
    service::CurrentUser,
    utils::{HarshIdEncoder, IdEncoder, OptimusIdEncoder, PrefixedIdEncoder},
};
use std::sync::Arc;
use tera::Tera;

pub struct AppSettings {
    pub super_user_api_key_hash: Option<String>,
}

struct Inner {
    settings: AppSettings,
    problem_config: ProblemConfig,
    tera: Tera,
    db: DBPool,
    telemetry_service: TelemetryService,
    identity_service: IdentityService<PgIdentityDb>,
    session_service: SessionService<RedisSessionDb>,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(config: &AppConfig, telemetry_service: &TelemetryService) -> Result<Self, AnyError> {
        let settings = AppSettings {
            super_user_api_key_hash: config.auth.super_user_api_key_hash.clone(),
        };
        let problem_config = ProblemConfig::new(config.service.full_problem_response);

        let tera = {
            let mut tera = Tera::new("tera_templates/**/*").map_err(|e| anyhow!(e))?;
            tera.autoescape_on(vec![".html"]);
            tera
        };

        let db_pool = DBPool::new(&config.db).await?;

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
            tera,
            db: db_pool,
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

    pub fn telemetry_service(&self) -> &TelemetryService {
        &self.0.telemetry_service
    }

    pub fn identity_service(&self) -> &IdentityService<impl IdentityDb> {
        &self.0.identity_service
    }

    pub fn session_service(&self) -> &SessionService<impl SessionDb> {
        &self.0.session_service
    }

    pub fn session_user_sync_service(&self) -> SessionUserSyncService<impl IdentityDb, impl SessionDb> {
        SessionUserSyncService::new(self.identity_service(), self.session_service())
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
