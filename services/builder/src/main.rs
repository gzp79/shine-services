mod app_config;
mod app_state;
mod controllers;
mod repositories;
mod services;

use self::{app_config::AppConfig, app_state::AppState};
use anyhow::Error as AnyError;
use controllers::{builder::BuilderController, health::HealthController};
use shine_core::web::{WebAppConfig, WebApplication};
use utoipa_axum::router::OpenApiRouter;

struct Application;

impl WebApplication for Application {
    type AppConfig = AppConfig;
    type AppState = AppState;

    fn feature_name(&self) -> &'static str {
        "builder"
    }

    async fn create_state(&self, config: &WebAppConfig<Self::AppConfig>) -> Result<Self::AppState, AnyError> {
        AppState::new(config).await
    }

    async fn create_routes(
        &self,
        _config: &WebAppConfig<Self::AppConfig>,
    ) -> Result<OpenApiRouter<Self::AppState>, AnyError> {
        let health_controller = HealthController::new().into_router();
        let builder_controller = BuilderController::new().into_router();

        Ok(health_controller.merge(builder_controller))
    }
}

pub fn main() {
    let app = Application;
    shine_core::web::run_web_app(app);
}
