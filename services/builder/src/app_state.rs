use crate::{
    app_config::AppConfig,
    repositories::command_stream::redis::{RedisCommandStreamDb, RedisStreamConfig},
    repositories::hub_registry::redis::RedisHubConnectionDb,
    services::{
        command_stream::{chat_access, StreamKindRegistry, StreamStores},
        CommandStreamService, HubIntervals, HubService,
    },
    settings::{BuilderSettings, CommandStreamSettings, WsSettings},
};
use anyhow::{anyhow, Error as AnyError};
use shine_game::replay::ChatState;
use shine_infra::{
    db::{command_log::PgCommandLogDb, postgres::PgConnectionPool, redis::RedisConnectionPool},
    web::{compile_anchored_bytes, CoreServices, WebAppConfig},
};
use std::{sync::Arc, time::Duration};

/// The hub's periodic cadences and the registry TTL derived from them.
#[derive(Debug, PartialEq, Eq)]
struct HubTiming {
    intervals: HubIntervals,
    connection_ttl_seconds: u64,
}

impl HubTiming {
    fn from_config(config: &AppConfig) -> Self {
        let heartbeat_seconds = config.hub_heartbeat_seconds.max(1);
        Self {
            // The hub owns its periodic connection consumers (heartbeat + session checker), each on
            // its own loop event-sourced from a Hub subscription.
            intervals: HubIntervals {
                heartbeat: Duration::from_secs(heartbeat_seconds),
                session_check: Duration::from_secs(config.auth_check_interval.max(1)),
            },
            // 3× heartbeat leaves room for a missed refresh (slow registry / deferred tick) before a
            // live connection's registry entry expires and it is wrongly reaped as stale.
            connection_ttl_seconds: heartbeat_seconds.saturating_mul(3),
        }
    }
}

struct Inner {
    hub_service: HubService,
    command_stream_service: Option<CommandStreamService>,
    settings: BuilderSettings,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(
        config: &WebAppConfig<AppConfig>,
        redis_pool: &RedisConnectionPool,
        pg_pool: Option<&PgConnectionPool>,
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
                command_stream: CommandStreamSettings::from_config(&config.feature.command_stream)?,
            }
        };

        let timing = HubTiming::from_config(&config.feature);
        let hub_registry = RedisHubConnectionDb::new(redis_pool, timing.connection_ttl_seconds).await?;
        let command_stream_service = match pg_pool {
            Some(pg_pool) => {
                let stores = StreamStores {
                    redis: RedisCommandStreamDb::new(
                        redis_pool,
                        RedisStreamConfig {
                            prefix: RedisCommandStreamDb::PREFIX.to_string(),
                            max_unpersisted: settings.command_stream.redis_max_unpersisted,
                            ttl_seconds: settings.command_stream.redis_ttl_seconds,
                        },
                    ),
                    log: PgCommandLogDb::new(pg_pool).await?,
                    settings: settings.command_stream.clone(),
                };
                let kinds = StreamKindRegistry::new().with::<ChatState>();
                Some(CommandStreamService::new(stores, kinds, Arc::new(chat_access())))
            }
            None => None,
        };

        let hub_service = HubService::new(
            hub_registry,
            core_services.current_user_service.clone(),
            timing.intervals,
            command_stream_service.clone(),
        )
        .await;

        Ok(Self(Arc::new(Inner {
            hub_service,
            command_stream_service,
            settings,
        })))
    }

    pub fn hub_service(&self) -> &HubService {
        &self.0.hub_service
    }

    pub fn command_stream_service(&self) -> Option<&CommandStreamService> {
        self.0.command_stream_service.as_ref()
    }

    pub fn settings(&self) -> &BuilderSettings {
        &self.0.settings
    }
}

#[cfg(test)]
#[path = "app_state_test.rs"]
mod app_state_test;
