mod app_config;
mod app_state;
mod controllers;
mod handlers;
mod repositories;
mod services;

use self::{
    app_config::AppConfig,
    app_state::AppState,
    controllers::{auth, health::HealthController, identity},
};
use anyhow::Error as AnyError;
use shine_infra::web::{WebAppConfig, WebApplication};
use utoipa_axum::router::OpenApiRouter;

struct Application {}

impl WebApplication for Application {
    type AppConfig = AppConfig;
    type AppState = AppState;

    async fn create_state(&self, config: &WebAppConfig<Self::AppConfig>) -> Result<Self::AppState, AnyError> {
        let state = AppState::new(config).await?;

        state.subscribe_user_info_handler().await;

        Ok(state)
    }

    async fn create_routes(
        &self,
        config: &WebAppConfig<Self::AppConfig>,
    ) -> Result<OpenApiRouter<Self::AppState>, AnyError> {
        let health_controller = HealthController::new().into_router();
        let identity_controller = identity::IdentityController::new().into_router();
        let auth_controller = auth::AuthController::new(config).await?.into_router();

        Ok(health_controller.merge(identity_controller).merge(auth_controller))
    }
}

pub fn main() {
    let app = Application {};
    shine_infra::web::run_web_app(app);
}
