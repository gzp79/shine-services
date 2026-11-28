use crate::{
    app_config::BuilderConfig,
    repositories::{
        command_inbox::redis::RedisCommandInboxDb, command_log::pg::PgCommandLogDb,
        hub_registry::redis::RedisHubConnectionDb,
    },
    services::{standard_registry, HubService},
};
use anyhow::Error as AnyError;
use shine_infra::{
    db::{postgres::PgConnectionPool, redis::RedisConnectionPool},
    web::{CoreServices, WebAppConfig},
};
use std::sync::Arc;

/// How long the commands of a stream wait in the inbox for a worker to take them.
const STREAM_INBOX_TTL_SECONDS: u64 = 60 * 60;

struct Inner {
    hub_service: HubService,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(
        config: &WebAppConfig<BuilderConfig>,
        redis_pool: &RedisConnectionPool,
        postgres_pool: &PgConnectionPool,
        core_services: &CoreServices,
    ) -> Result<Self, AnyError> {
        // 3× heartbeat leaves room for a missed refresh (slow registry / deferred tick) before a
        // live connection's registry entry expires and it is wrongly reaped as stale.
        let hub_connection_ttl_seconds = config.feature().hub.heartbeat.as_secs().saturating_mul(3);

        let hub_registry = RedisHubConnectionDb::new(redis_pool, hub_connection_ttl_seconds).await?;
        let command_inbox_db = RedisCommandInboxDb::new(redis_pool, STREAM_INBOX_TTL_SECONDS).await?;
        let command_log_db = PgCommandLogDb::new(postgres_pool).await?;

        let hub_service = HubService::builder(config.feature().hub.clone())
            .with_task_supervisor(core_services.task_supervisor.clone())
            .with_session_service(core_services.current_user_service.clone())
            .with_hub_registry(hub_registry)
            .with_stream_registry(|egress| {
                standard_registry(
                    command_inbox_db,
                    command_log_db,
                    egress,
                    config.feature().streams.clone(),
                )
            })?
            .build()
            .await?;

        Ok(Self(Arc::new(Inner { hub_service })))
    }

    pub fn hub_service(&self) -> &HubService {
        &self.0.hub_service
    }
}
