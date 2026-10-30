mod app_config;
mod app_state;
mod models;
mod repositories;
mod routes;
mod services;
mod settings;

#[cfg(test)]
pub mod testing;

use self::{app_config::AppConfig, app_state::AppState, services::HubStatus};
use anyhow::Error as AnyError;
use repositories::{create_postgres_pool, create_redis_pool};
use routes::ws::ws_routes;
use shine_infra::{
    db::{
        check_postgres_tls, check_redis_tls,
        postgres::PostgresPoolStatus,
        redis::{check_redis, RedisPoolStatus},
    },
    web::{AppBuildContext, FeatureConfig, WebAppConfig, WebApplication},
};
use utoipa_axum::router::OpenApiRouter;

struct Application;

impl WebApplication for Application {
    type AppConfig = AppConfig;
    type AppState = AppState;

    async fn create(
        &self,
        config: &WebAppConfig<Self::AppConfig>,
        context: &mut AppBuildContext<'_>,
        router: &mut OpenApiRouter<Self::AppState>,
    ) -> Result<Self::AppState, AnyError> {
        check_redis_tls(&config.core.stage, &config.feature.db.redis_cns)?;
        let redis_pool = create_redis_pool(&config.feature.db).await?;
        check_redis(&redis_pool).await?;

        check_postgres_tls(&config.core.stage, &config.feature.db.sql_cns)?;
        let postgres_pool = create_postgres_pool(&config.feature.db).await?;

        let state = AppState::new(config, &redis_pool, &postgres_pool, context.core_services()).await?;

        context.add_health_provider(PostgresPoolStatus::new(postgres_pool));
        context.add_health_provider(RedisPoolStatus::new(redis_pool));
        context.add_health_provider(HubStatus::new(state.hub_service().clone()));

        let hub_service = state.hub_service().clone();
        context.add_shutdown_hook(move || {
            if let Err(err) = hub_service.request_shutdown() {
                tracing::warn!("Failed to send hub shutdown command from shutdown hook: {err:#?}");
            }
        });

        *router = router.clone().nest(&format!("/{}", Self::AppConfig::NAME), ws_routes());

        Ok(state)
    }
}

pub fn main() {
    let app = Application;
    shine_infra::web::run_web_app(app);
}
