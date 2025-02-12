use crate::repositories::DBConfig;
use serde::{Deserialize, Serialize};
use shine_core::web::FeatureConfig;

/// The application configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub db: DBConfig,
}

impl FeatureConfig for AppConfig {
    const NAME: &'static str = "builder";
}
