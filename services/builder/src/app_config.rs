use crate::repositories::DBConfig;
use serde::{Deserialize, Serialize};

/// The application configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub builder_db: DBConfig,
}
