use crate::app_config::AppConfig;
use anyhow::Error as AnyError;
use shine_core::web::WebAppConfig;

#[derive(Clone)]
pub struct AppState();

impl AppState {
    pub async fn new(_config: &WebAppConfig<AppConfig>) -> Result<Self, AnyError> {
        Ok(Self())
    }
}
