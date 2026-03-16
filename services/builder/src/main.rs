mod app_config;
mod app_state;
mod controllers;
mod repositories;
mod services;

use self::{app_config::AppConfig, app_state::AppState};
use anyhow::Error as AnyError;
use controllers::builder::BuilderController;
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

        let builder_controller = BuilderController::new().into_router();
        let app_router = OpenApiRouter::new().merge(builder_controller);
        *router = router.clone().nest(&format!("/{}", Self::AppConfig::NAME), app_router);

        Ok(state)
    }
}

pub fn main() {
    let app = Application;
    shine_infra::web::run_web_app(app);
}
