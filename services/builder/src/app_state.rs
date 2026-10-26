use crate::{
    app_config::AppConfig,
    repositories::command_inbox::redis::RedisCommandInboxDb,
    repositories::command_log::pg::PgCommandLogDb,
    repositories::hub_registry::redis::RedisHubConnectionDb,
    services::{standard_registry, CommandStreamConfig, HubService, WorkerConfig},
    settings::{BuilderSettings, WsSettings},
};
use anyhow::{anyhow, Error as AnyError};
use shine_infra::{
    db::{postgres::PgConnectionPool, redis::RedisConnectionPool},
    web::{compile_anchored_bytes, CoreServices, WebAppConfig},
};
use std::sync::Arc;

/// How long the commands of a stream wait in the inbox for a worker to take them.
const STREAM_INBOX_TTL_SECONDS: u64 = 60 * 60;

struct Inner {
    hub_service: HubService,
    settings: BuilderSettings,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(
        config: &WebAppConfig<AppConfig>,
        redis_pool: &RedisConnectionPool,
        postgres_pool: &PgConnectionPool,
        core_services: &CoreServices,
    ) -> Result<Self, AnyError> {
        let config_ws = &config.feature.ws;

        let settings = {
            let allowed_origins = config
                .service
                .allowed_origins
                .iter()
                .map(|r| compile_anchored_bytes(r).map_err(|err| anyhow!("WebSocket origin config error: {err}")))
                .collect::<Result<Vec<_>, _>>()?;
            let allowed_hosts = config_ws
                .allowed_hosts
                .iter()
                .map(|r| compile_anchored_bytes(r).map_err(|err| anyhow!("WebSocket host config error: {err}")))
                .collect::<Result<Vec<_>, _>>()?;

            BuilderSettings {
                ws: WsSettings { allowed_origins, allowed_hosts },
            }
        };

        // 3× heartbeat leaves room for a missed refresh (slow registry / deferred tick) before a
        // live connection's registry entry expires and it is wrongly reaped as stale.
        let hub_connection_ttl_seconds = config.feature.hub.heartbeat.as_secs().saturating_mul(3);

        let hub_registry = RedisHubConnectionDb::new(redis_pool, hub_connection_ttl_seconds).await?;
        let command_inbox_db = RedisCommandInboxDb::new(redis_pool, STREAM_INBOX_TTL_SECONDS).await?;
        let command_log_db = PgCommandLogDb::new(postgres_pool).await?;

        let hub_service = HubService::builder(config.feature.hub.clone())
            .with_session_service(core_services.current_user_service.clone())
            .with_hub_registry(hub_registry)
            .with_stream_registry(|egress| {
                standard_registry(
                    command_inbox_db,
                    command_log_db,
                    egress,
                    CommandStreamConfig::default(),
                    WorkerConfig::default(),
                )
            })?
            .build()
            .await?;

        Ok(Self(Arc::new(Inner { hub_service, settings })))
    }

    pub fn hub_service(&self) -> &HubService {
        &self.0.hub_service
    }

    pub fn settings(&self) -> &BuilderSettings {
        &self.0.settings
    }
}
