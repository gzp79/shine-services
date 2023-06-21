use std::sync::Arc;

use crate::app_config::AppConfig;

struct Inner {
    home_url: String,
}

#[derive(Clone)]
pub struct SettingsManager(Arc<Inner>);

impl SettingsManager {
    pub fn new(config: &AppConfig) -> Self {
        Self(Arc::new(Inner {
            home_url: config.home_url.to_string(),
        }))
    }

    pub fn home_url(&self) -> &str {
        &self.0.home_url
    }
}
