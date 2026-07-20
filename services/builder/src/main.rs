mod app_config;
mod app_state;
mod models;
mod repositories;
mod routes;
mod services;
mod settings;

use self::{app_config::AppConfig, app_state::AppState};
use anyhow::Error as AnyError;
use models::messages::HubCommand;
use repositories::create_redis_pool;
use routes::ws::ws_routes;
use shine_infra::web::{AppBuildContext, FeatureConfig, WebAppConfig, WebApplication};
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
        let state = AppState::new(config, &redis_pool, context.core_services()).await?;

        let shutdown_sender = state.hub_service().sender();
        context.add_shutdown_hook(move || {
            if let Err(err) = shutdown_sender.send_command(HubCommand::Shutdown) {
                log::warn!("Failed to send hub shutdown command from shutdown hook: {err:#?}");
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
