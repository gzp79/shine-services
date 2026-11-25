mod app_config;
mod app_state;
mod models;
mod repositories;
mod routes;
mod services;

#[cfg(test)]
pub mod testing;

use self::{app_config::BuilderConfig, app_state::AppState, services::HubStatus};
use anyhow::Error as AnyError;
use repositories::{create_postgres_pool, create_redis_pool};
use routes::ws::ws_routes;
use shine_infra::{
    db::{
        postgres::PostgresPoolStatus,
        redis::{check_redis, RedisPoolStatus},
    },
    web::{AppBuildContext, FeatureConfig, WebAppConfig, WebApplication},
};
use utoipa_axum::router::OpenApiRouter;

struct Application;

impl WebApplication for Application {
    type AppConfig = BuilderConfig;
    type AppState = AppState;

    async fn create(
        &self,
        config: &WebAppConfig<Self::AppConfig>,
        context: &mut AppBuildContext<'_>,
        router: &mut OpenApiRouter<Self::AppState>,
    ) -> Result<Self::AppState, AnyError> {
        let redis_pool = create_redis_pool(&config.core().stage, &config.feature().db).await?;
        check_redis(&redis_pool).await?;

        let postgres_pool = create_postgres_pool(&config.core().stage, &config.feature().db).await?;

        let state = AppState::new(config, &redis_pool, &postgres_pool, context.core_services()).await?;

        context.add_health_provider(PostgresPoolStatus::new(postgres_pool));
        context.add_health_provider(RedisPoolStatus::new(redis_pool));
        context.add_health_provider(HubStatus::new(state.hub_service().clone()));

        let hub_service = state.hub_service().clone();
        context.add_shutdown_hook(move || async move {
            if let Err(err) = hub_service.shutdown().await {
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
