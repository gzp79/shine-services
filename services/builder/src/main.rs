mod app_config;
mod app_state;
mod models;
mod repositories;
mod routes;
mod services;
mod settings;

use self::{app_config::AppConfig, app_state::AppState};
use anyhow::Error as AnyError;
use routes::ws::ws_routes;
use shine_infra::{
    health::HealthService,
    web::{FeatureConfig, WebAppConfig, WebApplication},
};
use utoipa_axum::router::OpenApiRouter;

struct Application;

impl WebApplication for Application {
    type AppConfig = AppConfig;
    type AppState = AppState;

    async fn create(
        &self,
        config: &WebAppConfig<Self::AppConfig>,
        _health_service: &mut HealthService,
        router: &mut OpenApiRouter<Self::AppState>,
    ) -> Result<Self::AppState, AnyError> {
        let state = AppState::new(config).await?;

        *router = router.clone().nest(&format!("/{}", Self::AppConfig::NAME), ws_routes());

        Ok(state)
    }
}

pub fn main() {
    let app = Application;
    shine_infra::web::run_web_app(app);
}
