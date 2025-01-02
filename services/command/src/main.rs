mod app_config;
mod app_state;

use self::{app_config::AppConfig, app_state::AppState};
use anyhow::Error as AnyError;
use shine_core::web::{WebAppConfig, WebApplication};
use utoipa_axum::router::OpenApiRouter;

struct Application {}

impl WebApplication for Application {
    type AppConfig = AppConfig;
    type AppState = AppState;

    fn feature_name(&self) -> &'static str {
        "command"
    }

    async fn create_state(&self, config: &WebAppConfig<Self::AppConfig>) -> Result<Self::AppState, AnyError> {
        AppState::new(config).await
    }

    async fn create_routes(
        &self,
        _config: &WebAppConfig<Self::AppConfig>,
    ) -> Result<OpenApiRouter<Self::AppState>, AnyError> {
        Ok(OpenApiRouter::new())
    }
}

pub fn main() {
    let app = Application {};
    shine_core::web::run_web_app(app);
}
