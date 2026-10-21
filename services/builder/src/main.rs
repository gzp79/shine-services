mod app_config;
mod app_state;
mod models;
mod repositories;
mod routes;
mod services;
mod settings;

use self::{app_config::AppConfig, app_state::AppState, services::HubStatus};
use anyhow::Error as AnyError;
use repositories::{create_postgres_pool, create_redis_pool};
use routes::{command_stream::stream_routes, ws::ws_routes};
use shine_infra::{
    db::postgres::PostgresPoolStatus,
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
        let redis_pool = create_redis_pool(&config.feature.db).await?;

        // The builder's own Postgres database holds the command streams, so it is only needed with them.
        let pg_pool = if config.feature.command_stream.enabled {
            Some(create_postgres_pool(&config.feature.db).await?)
        } else {
            None
        };
        let state = AppState::new(config, &redis_pool, pg_pool.as_ref(), context.core_services()).await?;
        if let Some(pg_pool) = pg_pool {
            context.add_health_provider(PostgresPoolStatus::new(pg_pool));
        }

        context.add_health_provider(HubStatus::new(state.hub_service().clone()));

        let hub_service = state.hub_service().clone();
        context.add_shutdown_hook(move || {
            if let Err(err) = hub_service.request_shutdown() {
                tracing::warn!("Failed to send hub shutdown command from shutdown hook: {err:#?}");
            }
        });

        *router = router.clone().nest(
            &format!("/{}", Self::AppConfig::NAME),
            ws_routes().merge(stream_routes()),
        );

        Ok(state)
    }
}

pub fn main() {
    let app = Application;
    shine_infra::web::run_web_app(app);
}
